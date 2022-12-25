#!/usr/bin/env ruby

require 'rubygems'
require 'bundler/setup'

require 'sinatra'
require 'json'
require 'http'
require 'oj'


# before_action :require_actor_signature!

INBOX = []

require "active_support"
require "active_support/core_ext/object"
require "active_support/core_ext/enumerable"

require './webfinger.rb'

require './context_helper.rb'
require './webfinger_helper.rb'
require './activitypub/jsonld_helper.rb'
require './activitypub/tag_manager.rb'

require './signature_verification.rb'
include SignatureVerification



before do
  request.body.rewind
  @request_payload = request.body.read
  puts "PAYLOAD #{@request_payload}"
  #@request_payload = JSON.parse request.body.read
end

class Sinatra::Request
  attr_accessor :request_payload

  def raw_post
    @request_payload
  end
  
  def headers
    Hash[*env.select {|k,v| k.start_with? 'HTTP_'}
            .collect {|k,v| [k.sub(/^HTTP_/, ''), v]}
            .collect {|k,v| [k.split('_').collect(&:capitalize).join('-'), v]}
            .sort
            .flatten]
  end
end


get '/inspect' do
  [200, INBOX.join("\n\n")]
end

post '/inbox' do
  request.request_payload = @request_payload
  puts request.headers.inspect
  
  if signed_request_actor
    request.body.rewind
    INBOX << request.body.read
    [200, 'OK']
  else
    [401, 'Request signature could not be verified']
  end

  
  # signature_header = request.headers['Signature'].split(',').map do |pair|
  #   pair.split('=').map do |value|
  #     value.gsub(/\A"/, '').gsub(/"\z/, '') # "foo" -> foo
  #   end
  # end.to_h

  # key_id    = signature_header['keyId']
  # headers   = signature_header['headers']
  # signature = Base64.decode64(signature_header['signature'])

  # actor = JSON.parse(HTTP.get(key_id).to_s)
  # key   = OpenSSL::PKey::RSA.new(actor['publicKey']['publicKeyPem'])

  # comparison_string = headers.split(' ').map do |signed_header_name|
  #   if signed_header_name == '(request-target)'
  #     '(request-target): post /inbox'
  #   else
  #     "#{signed_header_name}: #{request.headers[signed_header_name.capitalize]}"
  #   end
  # end

  # if key.verify(OpenSSL::Digest::SHA256.new, signature, comparison_string)
  #   request.body.rewind
  #   INBOX << request.body.read
  #   [200, 'OK']
  # else
  #   [401, 'Request signature could not be verified']
  # end
end


