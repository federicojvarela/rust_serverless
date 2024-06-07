
```bash
./run_unit_and_integration_tests.sh [OPTIONS]
```
where `[OPTIONS]` are:
- `-v, --verbose`: Set verbosity. This also logs the `cargo lambda watch` output.

**NOTE**: It may appear that some integration tests take a long time, this is because cargo lambda compiles the lambda in the first invocation. Try to have all the dependencies compiled before running the tests!

**NOTE**: Before running the test, make sure that your local test environment file exist (`.env.test.local`) and that it contain the correct values. Normally you only need to change the IP addresses that points to the localstack services. An example of `.env.test.local` could be:
