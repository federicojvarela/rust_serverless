# MPC Signature State Machine

## Git Hooks
There are some git hooks located in the `./.githooks/` folder. To activate them you should run:

```bash
$ git config core.hooksPath .githooks
```

## Using Feature Flags locally

To use hardcoded in-memory feature flags:

- `FEATURE_FLAG_IN_MEMORY_MODE`: Only for development. Uses feature flags values from file [src/feature_flags/mod.rs](src/feature_flags/mod.rs). Default value is `false`.

Or, to retrieve from LaunchDarkly, add environment variables to `.env`:

- `AWS_REGION`: Only for development. AWS Already provides this env variable in lambda. AWS region for where the Secret Manager is located. Value is `us-west-2`.
- `AWS_ACCESS_KEY_ID`: Only for development. If using localstack, set to any value.
- `AWS_SECRET_ACCESS_KEY`: Only for development. If using localstack, set to any value.
- `LAUNCHDARKLY_SDK_KEY_SECRET_NAME`: Only for development. Name of the locally seeded secret that contains the LaunchDarkly's SDK key for feature flagging. Value is real secret name if retrieving from AWS or `launchdarkly_sdk_key_secret` if retrieving from localstack. Can be empty string if `FEATURE_FLAG_IN_MEMORY_MODE: true`.

To use the actual Secrets Manager you must also add the credential:

- `AWS_SESSION_TOKEN`: Only for development. Get it from AWS Console. Expires rather qiuckly.

Or, to use the **localstack** Secrets Manager you must add:

- `LOCALSTACK_TEST_MODE_ENDPOINT`: Only for development. The localstack endpoint. Default value: `None`.

## Running

To use the localstack Secrets Manager, you have to manually seed it and run it, then run cargo lambda watch:

```bash
SECRET_LAUNCHDARKLY_SDK_KEY =<LAUNCHDARKLY_SDK_KEY> docker-compose -f docker-compose-local.yml up
cargo lambda watch
```

### DynamoDB table explorer

A table explorer is run when running docker compose. To access it navigate to [http://127.0.0.1:4000](http://127.0.0.1:4000)


---

## Testing: Unit and Integration

### 1. Start localstack

First you have to run the localstack services by running the following command:

```bash
./start_local_containers.sh
```

**NOTE**:
If you want to test "what's in main, not local", you should run `docker-compose up` command instead.
If you do, sometimes you might get this error:
```bash
Error response from daemon: pull access denied for 575679510244.dkr.ecr.us-west-2.amazonaws.com/mpc-signature-sm-test-localstack, repository does not exist or may require 'docker login': denied: Your authorization token has expired. Reauthenticate and try again.
```
To fix this, try this:
1. Copy and paste your credentials from AWS: Command line or pragrammatic access -> Option 1: Set AWS environment variables (Short-term credentials)
2. Run this command:
```bash
aws ecr get-login-password --region us-west-2 | docker login --username AWS --password-stdin 575679510244.dkr.ecr.us-west-2.amazonaws.com
```
3. Then run `docker-compose up --build`.


### 2. Make sure you have `.env.test.local` file

If you don't, create a copy of `.env.test.local.example` file in the root of the project and name it `.env.test.local` (without the ".example" part).


### 3. Run Unit and Integration tests

While you have `start_local_containers` or `docker-compose up` running in one term tab/window, open another tab to run tests.

Run the bash file `run_unit_and_integration_tests.sh`. This script runs unit and integration tests:

```bash
./run_unit_and_integration_tests.sh [OPTIONS]
```
where `[OPTIONS]` are:
- `-v, --verbose`: Set verbosity. This also logs the `cargo lambda watch` output.

**NOTE**: It may appear that some integration tests take a long time, this is because cargo lambda compiles the lambda in the first invocation. Try to have all the dependencies compiled before running the tests!

**NOTE**: Before running the test, make sure that your local test environment file exist (`.env.test.local`) and that it contain the correct values. Normally you only need to change the IP addresses that points to the localstack services. An example of `.env.test.local` could be:

```dotenv
LOCALSTACK_TEST_MODE_ENDPOINT=http://127.0.0.1:4566
RESPONSE_QUEUE_URL=http://127.0.0.1:4566/000000000000/compliance-response
LAMBDA_WATCH_URL=http://127.0.0.1:9000
```

### 3. Run E2E tests

Create an ephemeral env if necessary (when testing code in a branch that modified AWS resources)

- **[DEPRECATED]** For bash E2E tests , In cli type:

```
cd /scripts
./run_e2e_tests.sh 
```
(or `./run_e2e_tests.sh wall-my-jira-number` for an eph env)


- For rust E2E tests see [E2E Readme](./e2e/README.md)

---

### Data seeds

Localstack is seeded through a series of scripts located in `./containers/localstack/`. There is a script for every service we need to seed, a healtcheck and an initialization script:

- `init.sh`: Set the needed environment variables and call all the seeding scripts.
- `healthcheck.sh`: Is used by the localstack container to check if all the services are up and running. Other services (such as the seeder container) need the localstack container to be healthly in order to run correctly.
- `<service>.sh`: Script that seeds a certain `<service>`.

To seed localstack, we use a container named `seeded-localstack` declared in the `docker-compose.yml` file. This container mounts the `./containers/localstack/` directory as a volume and executes the files from there. This mean that all the files present in the named folder will be present inside the container. This is very useful when we need to put the data that we are going to inject in separate files. That is the case of the `dynamodb.sh` script.

## Environment files

Environment files (`.env*`) are used to configure the lambdas. There are serveral env files:

- `.env.example`: Contains the default configurations required by the service to work locally. This file is not read, it just serves as an example.
- `.env`: Default environment file.
- `.env.test`: Test environment file. Used for running tests.

All environment files can be copied and appended with a `.local` suffix to override the values of the original `.env*` file. The files that are suffixed with `.local` are just for local development and should never be tracked in the version control.

## Test Input for State Machines

[Can be found here](./infrastructure/terraform/state_machines/README.md)

---


## Ephemeral Environments
Ephemeral environments are created as remote environments hosted on AWS. Outside of having different-looking URLs from traditional environments (e.g. dev, qa, staging, etc...) they are self-contained and behave just like a traditional, remote environment.

The lifecycle for working with an ephemeral environment looks as follows:

1. Create a local git branch for your ticket work
2. Create an ephemeral environment with provided scripts
3. Make local changes
4. Apply your changes to the ephemeral environment with provided scripts, which could include rebuilding and uploading lambdas
5. Test the ephemeral environment manually or with provided script
6. Create a PR with your changes and a reference to your ephemeral environment
7. Go through the code review cycle, applying changes to your ephemeral environment as you work through issues
8. Before merging your PR to main, destroy your ephemeral environment

Scripts have been provided for all the ephemeral environment operations shown below. Each script depends on a unique identifier that gets added to all the top-level Terraform resources and so it's important to keep the value unique, for example, using Jira ticket codes like `wall-100`. You will need to keep track of this unique value since you should use the same value across all the scripts. The length of the unique value should be as short as possible since the unique value becomes a part of the AWS resource name/id, which can have length constraints depending on the AWS service.

**NOTE:** Your AWS credentials expire after one hour, and so you will see a message like this if your work session lasts longer and you try to update your ephemeral environment: `ExpiredToken: The security token included in the request is expired`. You'll need to get new credentials from the Forte Okta AWS tile, using the `aws-aa-wallet-development` account, or preferrably from using `aws sso login` (which avoids having to cut and paste credentials). See the AWS login section below for more info.

### Requirements
You must have the following shell commands installed
- `jq`: can be installed with brew (`brew install jq`), used for parsing JSON responses
- `base64`: installed by default on MacOS, but you can also install with brew (`brew install base64`)
  - must support `-i` and `--decode` options (MacOS version does)
- `aws`: can be installed with brew (`brew install aws-cli`), used for interacting with AWS
- `terraform`: can be installed with brew (`brew install terraform`)
- `openssl`: must be version 3.2.x, use `openssl version` to find out. The brew version (`brew install openssl`) works well
- `gh`: can be installed with brew (`brew install gh`)

**Additionally, you will need to log in to your AWS wallet dev account and your github account as described in the following sections**

#### AWS login

You will need to log in to the AWS wallet dev account in order to create ephemeral environments. To do that, you'll need to complete a few tasks...

1. Create or edit `~/.aws/config` and add the following configuration block:
```bash
[profile ephemeral]
sso_account_id = 267505102317
sso_role_name = DeveloperAccess
region = us-west-2
output = json
sso_start_url = https://d-9067b7c12b.awsapps.com/start
sso_region = us-east-1
sso_registration_scopes = sso:account:access
```
2. Run `aws sso login` to log into your account

#### Github login

You will need to log in to your Github account in order for Terraform to dowload Terraform modules from the `aa-iac-aws.git` repo. To do that, you'll need to complete a few tasks...

```bash
$ gh auth login
What account do you want to log into? GitHub.com
What is your preferred protocol for Git operations? HTTPS
Authenticate Git with your GitHub credentials? Yes
How would you like to authenticate GitHub CLI? Login with a web browser

First copy your one-time code: XXXX-XXXX
Press Enter to open github.com in your browser...
✓ Authentication complete.
- gh config set -h github.com git_protocol https
✓ Configured git protocol
✓ Logged in as [your account name shown here]
```

### Create an ephemeral environment
1. Change your working directory to the $REPO_ROOT/scripts directory
2. Execute `./create_env.sh [unique value]`, where the value could be a value like `wall-503` or `ad-testing-1`

**NOTE:** The environment takes several minutes to get created and there is a race condition where if you get an error about cognito client ids (`Error: creating Cognito User Pool Client`), you will need to run `./update_env.sh [same unique value]` to complete the setup

### Update an ephemeral environment
1. Change your working directory to the $REPO_ROOT/scripts directory
2. Execute `./update_env.sh [same unique value]`

**NOTE:** If you made Rust changes to your lambdas, you must rebuild and upload your lambdas BEFORE updating the ephemeral environment

### Rebuild and upload lambdas
1. Change your working directory to the $REPO_ROOT/scripts directory
2. Execute `./build_lambdas.sh [same unique value]`
3. Execute `./update_env.sh [same unique value]`

### Destroy an ephemeral environment
1. Change your working directory to the $REPO_ROOT/scripts directory
2. Execute `./destroy_env.sh [same unique value]`

### Run happy path tests on an ephemeral environment
1. Change your working directory to the $REPO_ROOT/scripts directory
2. Execute `./run_e2e_tests.sh [same unique value]`

### Tips
- If you change a lambda, you will need to run both the `build_lambdas.sh` script, followed by the `update_env.sh` script
- If you change a Terraform resource (e.g. step function, apig, dynamodb), you only need to run the `update_env.sh` script
- Most scripts are re-entrant, meaning that if you get a failure running a script, you should try again in case the error was a temporary AWS issue. However, if you are creating an environment and it fails during the Terraform apply state, it's much faster to run the `update_env.sh` script because it doesn't rebuild lambdas or create any secrets

### Troubleshooting terraform
Terraform can return odd errors from time to time. This list includes all the errors we've encountered so far and how we've resolved them...
#### **Locked provider error**
If you see an error similar to the one below...
```
│ Error: Failed to query available provider packages
│
│ Could not retrieve the list of available versions for provider hashicorp/aws: locked provider registry.terraform.io/hashicorp/aws 4.56.0 does
│ not match configured version constraint >= 3.27.0, >= 4.9.0, >= 4.23.0, ~> 4.65; must use terraform init -upgrade to allow selection of new
│ versions
```
...delete your local Terraform lock file at `infrastructure/terraform/.terraform.lock.hcl` and run Terraform again

#### **Cannot read .aws/config**
If you see an error similar to the one below...
```
│ Error: configuring Terraform AWS Provider: failed to load shared config file, ~/.aws/config, invalid state with ASTKind  and TokenType none
```
...check your `~/.aws/config` and verify that you don't have configurations that don't have any value, as in...
```
[default]
cli_pager =
```

#### **Missing SSO configuration values**
If you see an error similar to the one below...
```
Missing the following required SSO configuration values: sso_start_url, sso_region. To make sure this profile is properly configured to use SSO, please run: aws configure sso
```
...try running the sso login by specifying the profile: `aws sso login --profile ephemeral`

You can also add `export AWS_PROFILE=ephemeral` to your `.zshrc` or `.bashrc` file.

#### **Invalid resource type error**
If you see an error similar to the one below...
```
Error: Invalid resource type
│
│   on event_bridge.tf line 78, in resource "aws_pipes_pipe" "compliance_request_pipe":
│   78: resource "aws_pipes_pipe" "compliance_request_pipe" {
│
│ The provider hashicorp/aws does not support resource type "aws_pipes_pipe".
```
...delete your `infrastructure/terraform/.terraform/providers` directory and the `infrastructure/terraform/.terraform.lock.hcl` file and run Terraform again

#### **Invalid lambda index error**
If you see an error similar to the one below...
```
│ Error: Invalid index
│
│   on api-gateway.tf line 281, in resource "aws_api_gateway_authorizer" "lambda_authorizer":
│  281:   authorizer_uri         = module.lambda["${var.prefix_env}-apikey_query_authorizer"].lambda_function_invoke_arn
│     ├────────────────
│     │ module.lambda is object with 8 attributes
│     │ var.prefix_env is "wall-309"
│
│ The given key does not identify an element in this collection value.
```
...run the `scripts/build_lambdas.sh` script to rebuild the lambda json and run the script that failed originally again

#### **Unbound variable**
If you see an error similar to the one below...
```
./create_env.sh: line 41: secret_arn_maestro_api_key: unbound variable
```
That could happen if you don't have secrets created for some reason. Try running these commands, it might help:
```
./destroy_env.sh wall-412
./manage_secrets.sh restore wall-412
./create_env.sh wall-412
```

#### Manual Approver UI

If you need to test using a policy that involves the Manual Approver on Dev/Staging you can use the UI that lives
here: https://special-adventure-p879yqj.pages.github.io/

You will need to choose your environment in the nav bar dropdown and you will need to login using credentials that
can be provided to you by Austin, Fran, or Karabo for now. 
