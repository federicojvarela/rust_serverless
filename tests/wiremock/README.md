Wiremock is used as a standalone mock server for integration tests.

### Mocks
The `mappings` folder defines all the mocked responses that will be served by wiremock. More info about matchers and
syntax can be found [here](https://wiremock.org/docs/request-matching/).

Output from wiremock is shown in the console so if a matcher or mock is not working as expected take a look at the 
terminal where you are running the `docker-compose` for feedback.

### Ganache Proxy
Our API interacts with a blockchain provider (Alchemy) in two ways:
1. EVM standard calls (like `eth_getBalance`).
2. Non EVM standard calls (like `getNFTs`).

We could use Wiremock to mock all Alchemy calls. However, for the EVM standard ones, we were previously using Ganache 
since it provides an implementation that it's closer to the real thing. This means that we need both wiremock and 
ganache running. The problem with this approach is that the alchemy endpoint is provided through env variables. In
real envs it's only one value (alchemy) but in local/tests we need two values (wiremock and ganache) which is not 
possible. To bypass this and avoid adding production code that it's only for testing, a special mapping is defined
for ganache so that those requests are redirected to ganache instead of looking for a Wiremock mocked response. 

