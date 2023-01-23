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

    if ( response.status == 404 ) {
      console.log("No feed there");
    } else if ( response.status > 200 ) {
      console.log("Something went wrong");
    } else {
      const data = await response.json();
      console.log(data);

      document.querySelector(".add-feed input[name='url']").value = data.url;
      document.querySelector(".add-feed").attributes.action = "/feed";

      button.removeEventListener("click", checkForValidFeed);
      button.type = "submit";

      document.querySelector(".add-feed-results").innerHTML = "Looks good!";
    }
  };

  button.addEventListener("click", checkForValidFeed);
}