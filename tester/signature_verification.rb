# frozen_string_literal: true

require 'parslet'
require './request.rb'
require './activitypub/base_service.rb'
require './activitypub/fetch_remote_actor_service.rb'
require './activitypub/fetch_remote_key_service.rb'


module Mastodon; end


# Implemented according to HTTP signatures (Draft 6)
# <https://tools.ietf.org/html/draft-cavage-http-signatures-06>
module SignatureVerification
  # extend ActiveSupport::Concern

  # include DomainControlHelper

  EXPIRATION_WINDOW_LIMIT = 12 * 3600 # 12.hours
  CLOCK_SKEW_MARGIN       = 3600 # 1.hour

  class SignatureVerificationError < StandardError; end
  class Mastodon::PrivateNetworkAddressError < StandardError; end
  class Mastodon::HostValidationError < StandardError; end
  class ActivityPub::FetchRemoteActorService::Error < StandardError; end
  class ActivityPub::FetchRemoteKeyService::Error < StandardError; end
  # class Webfinger::Error => e

  
  class SignatureParamsParser < Parslet::Parser
    rule(:token)         { match("[0-9a-zA-Z!#$%&'*+.^_`|~-]").repeat(1).as(:token) }
    rule(:quoted_string) { str('"') >> (qdtext | quoted_pair).repeat.as(:quoted_string) >> str('"') }
    # qdtext and quoted_pair are not exactly according to spec but meh
    rule(:qdtext)        { match('[^\\\\"]') }
    rule(:quoted_pair)   { str('\\') >> any }
    rule(:bws)           { match('\s').repeat }
    rule(:param)         { (token.as(:key) >> bws >> str('=') >> bws >> (token | quoted_string).as(:value)).as(:param) }
    rule(:comma)         { bws >> str(',') >> bws }
    # Old versions of node-http-signature add an incorrect "Signature " prefix to the header
    rule(:buggy_prefix)  { str('Signature ') }
    rule(:params)        { buggy_prefix.maybe >> (param >> (comma >> param).repeat).as(:params) }
    root(:params)
  end

  class SignatureParamsTransformer < Parslet::Transform
    rule(params: subtree(:param)) do
      (param.is_a?(Array) ? param : [param]).each_with_object({}) { |(key, value), hash| hash[key] = value }
    end

    rule(param: { key: simple(:key), value: simple(:val) }) do
      [key, val]
    end

    rule(quoted_string: simple(:string)) do
      string.to_s
    end

    rule(token: simple(:string)) do
      string.to_s
    end
  end

  def require_account_signature!
    render plain: signature_verification_failure_reason, status: signature_verification_failure_code unless signed_request_account
  end

  def require_actor_signature!
    render plain: signature_verification_failure_reason, status: signature_verification_failure_code unless signed_request_actor
  end

  def signed_request?
    request.headers['Signature'].present?    
  end

  def signature_verification_failure_reason
    @signature_verification_failure_reason
  end

  def signature_verification_failure_code
    @signature_verification_failure_code || 401
  end

  def signature_key_id
    signature_params['keyId']
  rescue SignatureVerificationError
    nil
  end

  def signed_request_account
    signed_request_actor.is_a?(Account) ? signed_request_actor : nil
  end

  def signed_request_actor
    puts "YO =========================="
    # return @signed_request_actor if defined?(@signed_request_actor)

    puts "Is request signed??? #{signed_request?}"
    raise SignatureVerificationError, 'Request not signed' unless signed_request?

    puts "Are required signature params present? #{!missing_required_signature_parameters?}"
    raise SignatureVerificationError, 'Incompatible request signature. keyId and signature are required' if missing_required_signature_parameters?

    puts "Is the signature algorithm good? #{%w(rsa-sha256 hs2019).include?(signature_algorithm)}"
    raise SignatureVerificationError, 'Unsupported signature algorithm (only rsa-sha256 and hs2019 are supported)' unless %w(rsa-sha256 hs2019).include?(signature_algorithm)

    puts "Is the time window acceptable??? #{matches_time_window?}"
    raise SignatureVerificationError, 'Signed request date outside acceptable time window' unless matches_time_window?

    puts "Verify strength"
    verify_signature_strength!

    puts "Verify digest"
    verify_body_digest!

    puts "We made it past the digest check!!!!"

    puts "Get actor from keyId" 
    #actor = actor_from_key_id(signature_params['keyId'])

    #puts "Do we have an actor???? #{actor.present?}"
    #raise SignatureVerificationError, "Public key not found for key #{signature_params['keyId']}" if actor.nil?

    # hack in keypair here
    if File.exist?('../feed_rsa.pub')
      keypair = OpenSSL::PKey::RSA.new(File.read('../feed_rsa.pub'))
    end
    
    signature             = Base64.decode64(signature_params['signature'])
    compare_signed_string = build_signed_string

    
    #result = verify_signature(actor, signature, compare_signed_string)
    result = verify_signature_from_keypair?(keypair, signature, compare_signed_string)
    puts "Verify result???? #{result}"


    return result

    return actor unless result.nil?

    actor = stoplight_wrap_request { actor_refresh_key!(actor) }

    raise SignatureVerificationError, "Public key not found for key #{signature_params['keyId']}" if actor.nil?

    return actor unless verify_signature(actor, signature, compare_signed_string).nil?

    fail_with! "Verification failed for #{actor.to_log_human_identifier} #{actor.uri} using rsa-sha256 (RSASSA-PKCS1-v1_5 with SHA-256)"
  rescue SignatureVerificationError => e
    fail_with! e.message
  rescue HTTP::Error, OpenSSL::SSL::SSLError => e
    fail_with! "Failed to fetch remote data: #{e.message}"
  end

  def request_body    
    @request_body ||= request.raw_post
  end

  private

  def fail_with!(message)
    @signature_verification_failure_reason = message
    @signed_request_actor = nil
  end

  def signature_params
    @signature_params ||= begin
      raw_signature = request.headers['Signature']
      tree          = SignatureParamsParser.new.parse(raw_signature)
      SignatureParamsTransformer.new.apply(tree)
    end
  rescue Parslet::ParseFailed
    raise SignatureVerificationError, 'Error parsing signature parameters'
  end

  def signature_algorithm
    signature_params.fetch('algorithm', 'hs2019')
  end

  def signed_headers
    signature_params.fetch('headers', signature_algorithm == 'hs2019' ? '(created)' : 'date').downcase.split(' ')
  end

  def verify_signature_strength!
    raise SignatureVerificationError, 'Mastodon requires the Date header or (created) pseudo-header to be signed' unless signed_headers.include?('date') || signed_headers.include?('(created)')
    raise SignatureVerificationError, 'Mastodon requires the Digest header or (request-target) pseudo-header to be signed' unless signed_headers.include?(Request::REQUEST_TARGET) || signed_headers.include?('digest')
    raise SignatureVerificationError, 'Mastodon requires the Host header to be signed when doing a GET request' if request.get? && !signed_headers.include?('host')
    raise SignatureVerificationError, 'Mastodon requires the Digest header to be signed when doing a POST request' if request.post? && !signed_headers.include?('digest')
  end

  def verify_body_digest!
    puts "Did we get a digest???? #{signed_headers.include?('digest')}"
    return unless signed_headers.include?('digest')

    puts request.headers.inspect
    raise SignatureVerificationError, 'Digest header missing' unless request.headers.key?('Digest')

    digests = request.headers['Digest'].split(',').map { |digest| digest.split('=', 2) }.map { |key, value| [key.downcase, value] }
    sha256  = digests.assoc('sha-256')
    raise SignatureVerificationError, "Mastodon only supports SHA-256 in Digest header. Offered algorithms: #{digests.map(&:first).join(', ')}" if sha256.nil?

    return if body_digest == sha256[1]

    digest_size = begin
      Base64.strict_decode64(sha256[1].strip).length
    rescue ArgumentError
      raise SignatureVerificationError, "Invalid Digest value. The provided Digest value is not a valid base64 string. Given digest: #{sha256[1]}"
    end

    raise SignatureVerificationError, "Invalid Digest value. The provided Digest value is not a SHA-256 digest. Given digest: #{sha256[1]}" if digest_size != 32
    raise SignatureVerificationError, "Invalid Digest value. Computed SHA-256 digest: #{body_digest}; given: #{sha256[1]}"
  end

  def verify_signature(actor, signature, compare_signed_string)
    if actor.keypair.public_key.verify(OpenSSL::Digest.new('SHA256'), signature, compare_signed_string)
      @signed_request_actor = actor
      @signed_request_actor
    end
  rescue OpenSSL::PKey::RSAError
    nil
  end

  def verify_signature_from_keypair?(keypair, signature, compare_signed_string)
    keypair.public_key.verify(OpenSSL::Digest.new('SHA256'), signature, compare_signed_string)
  rescue OpenSSL::PKey::RSAError => ex
    puts ex
    false
  end

  def build_signed_string
    signed_headers.map do |signed_header|
      puts signed_header
      if signed_header == Request::REQUEST_TARGET
        #"#{Request::REQUEST_TARGET}: #{request.method.downcase} #{request.path}"
        "#{Request::REQUEST_TARGET}: post #{request.path}"        
      elsif signed_header == '(created)'
        raise SignatureVerificationError, 'Invalid pseudo-header (created) for rsa-sha256' unless signature_algorithm == 'hs2019'
        raise SignatureVerificationError, 'Pseudo-header (created) used but corresponding argument missing' if signature_params['created'].blank?

        "(created): #{signature_params['created']}"
      elsif signed_header == '(expires)'
        raise SignatureVerificationError, 'Invalid pseudo-header (expires) for rsa-sha256' unless signature_algorithm == 'hs2019'
        raise SignatureVerificationError, 'Pseudo-header (expires) used but corresponding argument missing' if signature_params['expires'].blank?

        "(expires): #{signature_params['expires']}"
      else
        "#{signed_header}: #{request.headers[to_header_name(signed_header)]}"
      end
    end.join("\n")
  end

  def matches_time_window?
    created_time = nil
    expires_time = nil

    begin
      if signature_algorithm == 'hs2019' && signature_params['created'].present?
        created_time = Time.at(signature_params['created'].to_i).utc
      elsif request.headers['Date'].present?
        created_time = Time.httpdate(request.headers['Date']).utc
      end

      expires_time = Time.at(signature_params['expires'].to_i).utc if signature_params['expires'].present?
    rescue ArgumentError
      return false
    end

    ##expires_time ||= created_time + 5.minutes unless created_time.nil?
    expires_time ||= created_time + 300 unless created_time.nil?    
    expires_time = [expires_time, created_time + EXPIRATION_WINDOW_LIMIT].min unless created_time.nil?

    return false if created_time.present? && created_time > Time.now.utc + CLOCK_SKEW_MARGIN
    return false if expires_time.present? && Time.now.utc > expires_time + CLOCK_SKEW_MARGIN

    true
  end

  def body_digest
    @body_digest ||= Digest::SHA256.base64digest(request_body)
  end

  def to_header_name(name)
    name.split(/-/).map(&:capitalize).join('-')
  end

  def missing_required_signature_parameters?
    signature_params['keyId'].blank? || signature_params['signature'].blank?
  end

  def domain_not_allowed?(domain)
    false
  end
  
  def actor_from_key_id(key_id)
    domain = key_id.start_with?('acct:') ? key_id.split('@').last : key_id

    puts "actor_from_key_id: KEY #{key_id} DOMAIN #{domain}"
    
    if domain_not_allowed?(domain)
      @signature_verification_failure_code = 403
      return
    end

    if key_id.start_with?('acct:')
      ResolveAccountService.new.call(key_id.gsub(/\Aacct:/, ''), suppress_errors: false)
    #elsif !ActivityPub::TagManager.instance.local_uri?(key_id)
    else
      # this is the cached lookup
      #account   = ActivityPub::TagManager.instance.uri_to_actor(key_id)
      account = ActivityPub::FetchRemoteKeyService.new.call(key_id, id: false, suppress_errors: false)
      account
    end
  rescue Mastodon::PrivateNetworkAddressError => e
    raise SignatureVerificationError, "Requests to private network addresses are disallowed (tried to query #{e.host})"
  rescue Mastodon::HostValidationError, ActivityPub::FetchRemoteActorService::Error, ActivityPub::FetchRemoteKeyService::Error, Webfinger::Error => e
    raise SignatureVerificationError, e.message
  end

  def stoplight_wrap_request(&block)
    Stoplight("source:#{request.remote_ip}", &block)
      .with_threshold(1)
      .with_cool_off_time(5.minutes.seconds)
      .with_error_handler { |error, handle| error.is_a?(HTTP::Error) || error.is_a?(OpenSSL::SSL::SSLError) ? handle.call(error) : raise(error) }
      .run
  end

  def actor_refresh_key!(actor)
    return if actor.local? || !actor.activitypub?
    return actor.refresh! if actor.respond_to?(:refresh!) && actor.possibly_stale?

    ActivityPub::FetchRemoteActorService.new.call(actor.uri, only_key: true, suppress_errors: false)
  #rescue Mastodon::PrivateNetworkAddressError => e
  #  raise SignatureVerificationError, "Requests to private network addresses are disallowed (tried to query #{e.host})"
  rescue Mastodon::HostValidationError, ActivityPub::FetchRemoteActorService::Error, Webfinger::Error => e
    raise SignatureVerificationError, e.message
  end
end
