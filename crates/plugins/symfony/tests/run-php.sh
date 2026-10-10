#!/bin/sh
# Local runner for the ignored Rust integration probe. A Docker wrapper can be
# supplied instead through POOLSTER_SYMFONY_RUNNER when PHP is not installed.
set -eu
root="$1"
style="$2"
: "${POOLSTER_SYMFONY_VENDOR:?Install the locked fixture and set its vendor path}"
: "${POOLSTER_SYMFONY_COMPOSER:?Path to composer.phar}"
php_bin="${POOLSTER_TEST_PHP:-php}"
cd "$root"
"$php_bin" "$POOLSTER_SYMFONY_COMPOSER" dump-autoload --no-interaction --no-scripts
find src -name '*.php' -print0 | xargs -0 -n1 "$php_bin" -l
"$php_bin" probe.php "$style"
