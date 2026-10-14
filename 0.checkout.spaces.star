"""
Checkout the spaces sdk/package repos
"""

load("//@star/prelude/rules/checkout.star", "checkout_add_env_vars", "checkout_add_repo")
load("//@star/prelude/rules/env.star", "env_append")
load("//@star/prelude/rules/sdk.star", "sdk_add_repo")

def pre_checkout():
    # Ensure tools checked out to sysroot/bin are available
    # during checkout_add_exec() calls
    checkout_add_env_vars(
        "sysroot_env_path",
        vars = [
            env_append("PATH", "{}/sysroot/bin".format(workspace.get_absolute_path()), help = "Add sysroot/bin to the PATH"),
        ],
    )

    checkout_add_repo(
        "@star/packages",
        url = "https://github.com/work-spaces/packages",
        rev = "add-package-tracking",
    )

sdk_add_repo(
    "@star/sdk",
    url = "https://github.com/work-spaces/sdk",
    rev = "main",
)
