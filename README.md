# Home-Web

Homeweb is a fast muti-threaded RFC compliant mdns library made for tokio runtime. It's compliant with **RFC-6762** and also **RFC-6763**. The API is very simple but behind the scene it's 
intelligently handling every aspect with a lot of optimizations for being able to be used with other applications and remaining light weight itself.

# Current Features

- The library runtime depends on tokio worker tasks for parallely responding to other's queries and intelligently caching responses got from others reducing the cpu usage.
- It uses an interesting algorithm that can identify which responses the client is really interested about depending on previous query history and only save the interersted query responses.