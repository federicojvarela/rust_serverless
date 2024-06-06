# E2E Tests

This repository contains end-to-end (E2E) tests for MPC. These tests ensure that different components of our application interact correctly, simulating real-world scenarios.

## Test Naming Convention

All E2E tests should adhere to the following naming convention:

- Prefix: `e2e_test_`
- Example: `e2e_test_check_endpoint_example`

## Process Indicators

Throughout the tests, you'll notice the following process indicators in the log output:

- `[+]`: Start of a process.
- `[~]`: Awaiting a condition or event.
- `[!]`: Highlights data output, important notices or warnings during the test execution.
- `[=]`: Initiation of validation or assertion.
- `[-]`: End of a process.

These indicators provide a clear visual representation of the flow and status of the test operations.

E.G.:
```shell
   ...
   [11:18:07.044] [+] [Get order status] - Validates Order Status
   [11:18:07.044] [!] [Get order status] - OrderID: 6a594782-5f3c-4218-82b4-2aed91e7eaba
   [11:18:07.352] [~] [Get order status] - State is "RECEIVED", waiting for ERROR or NOT_SUBMITTED or COMPLETED
   [11:18:08.598] [~] [Get order status] - State is "SIGNED", waiting for ERROR or NOT_SUBMITTED or COMPLETED
   [11:18:09.913] [~] [Get order status] - State is "SUBMITTED", waiting for ERROR or NOT_SUBMITTED or COMPLETED
   [11:18:11.161] [~] [Get order status] - State is "SUBMITTED", waiting for ERROR or NOT_SUBMITTED or COMPLETED
   [11:18:12.473] [~] [Get order status] - State is "SUBMITTED", waiting for ERROR or NOT_SUBMITTED or COMPLETED
   [11:18:13.732] [!] [Get order status] - State is "COMPLETED"
   [11:18:13.732] [=] [Get order status] - Assert Response
   [11:18:13.732] [-] [Get order status] - Validates Order Status
   ...
```

## Running the Tests

To run the E2E tests, follow these steps:

1. Ensure you have Rust installed on your machine.
2. Open a terminal and navigate to the root directory of the project.
3. Login to the AWS DEV environment.
4. Run the following command:
```
./run_e2e_tests.sh -e $ENV -j $JOBS 
```
The `-j` argument is optional and allows you to configure the number of parallel jobs to run. Defaults to the number of logical CPUs.

Example for dev:
```sh
./run_e2e_tests.sh -e dev 
```

Internally run_e2e_tests.sh will run the following command:

```sh
ENV=dev cargo test -- --nocapture 
```

For ephemeral environments
```sh
   ENV=wall-123 cargo test -- --nocapture
```

### Explanation of the command above

This command runs all of the end-to-end (E2E) tests in the current project using the `dev` profile. The E2E tests are located in the `e2e_tests` module. The `--nocapture` flag tells Cargo to print the output of the tests to the terminal.

The `ENV` environment variable can be set to one of the following values:

* `local`: This will run the tests in a local environment.
* `dev`: This will run the tests in a development environment.
* `staging`: This will run the tests in a staging environment.

Here is a breakdown of each of the components of the command:

* `ENV`: This environment variable is used to pass environment variables to the Rust compiler and Cargo.
* `cargo test`: This command runs all of the tests in the current project.
* `--`: This argument tells Cargo to ignore any other arguments that are passed to the command. This is useful when you want to pass additional arguments to the test binary, such as the `--nocapture` argument.
* `--nocapture`: This argument tells Cargo to print the output of the tests to the terminal. This is useful for debugging tests.
