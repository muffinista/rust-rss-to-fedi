let button = document.querySelector(".add-feed button.check");
if (button) {
  const checkForValidFeed = async (event) => {
    event.preventDefault();

    const messageDest = document.querySelector(".add-feed-results");
    const name = document.querySelector(".add-feed input[name='name']").value;
    const url = document.querySelector(".add-feed input[name='url']").value;
    const payload = {
      name, url
    }

    if (!name.match(/^[a-z0-9]/i) ) {
      messageDest.innerHTML = "Sorry, please limit the username to letters and digits";
      return;
    }

    messageDest.innerHTML = "Checking that feed is valid..."; 

    const response = await fetch('/test-feed', {
      method: 'POST',
      body: JSON.stringify(payload),
    });

    if ( response.status == 404 ) {
      console.log("No feed there");
      messageDest.innerHTML = "Sorry, we couldn't find a valid feed at that URL";
    } else if ( response.status > 200 ) {
      console.log("Something went wrong");
      messageDest.innerHTML = "Sorry, something went wrong";
    } else {
      const data = await response.json();
      console.log(data);

      if ( data.error ) {
        messageDest.innerHTML = data.error;  
      } else {
        document.querySelector(".add-feed input[name='url']").value = data.url;
        document.querySelector(".add-feed").attributes.action = "/feed";
  
        button.removeEventListener("click", checkForValidFeed);
        button.type = "submit";
  
        messageDest.innerHTML = "Looks good! Click 'Add feed' one more time to create the account";  
      }
    }
  };

  button.addEventListener("click", checkForValidFeed);
}

function copy(target) {
  target.querySelector(".copy-target").select();
  document.execCommand("copy");

  let zinger = target.querySelector("small");
  zinger.classList.remove("hidden");
  setTimeout(() => {
    zinger.classList.add("hidden")
  }, 2000);
}

document.querySelectorAll(".copy-block").forEach((el) => {
  let adminAddress = el.querySelector(".copy-target");
 
  console.log(adminAddress);
  if (adminAddress) {
    adminAddress.addEventListener("click", (event) => {
      copy(el);
    });
    el.querySelector("a").addEventListener("click", () => { copy(el); });
  }  
});
