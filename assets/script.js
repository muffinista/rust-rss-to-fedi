let button = document.querySelector(".add-feed button.check");
if (button) {
  const checkForValidFeed = async (event) => {
    event.preventDefault();

    const name = document.querySelector(".add-feed input[name='name']").value;
    const url = document.querySelector(".add-feed input[name='url']").value;
    const payload = {
      name, url
    }

    const response = await fetch('/test-feed', {
      method: 'POST',
      body: JSON.stringify(payload),
    });

    const messageDest = document.querySelector(".add-feed-results");

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
  
        messageDest.innerHTML = "Looks good! Click ok to create account";  
      }
    }
  };

  button.addEventListener("click", checkForValidFeed);
}