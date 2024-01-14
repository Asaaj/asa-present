#!/bin/bash

set -euv -o pipefail

#channels_to_build="${CHANNELS_TO_BUILD-stable beta nightly}"
channels_to_build="${CHANNELS_TO_BUILD-stable}"

repository=asa-present

for channel in $channels_to_build; do
    cd "rust-base"

    image_name="rust-${channel}"
    full_name="${repository}/${image_name}"

    docker build -t "${full_name}" \
           --build-arg channel="${channel}" \
           .

    docker tag "${full_name}" "${image_name}"

    cd ..
done
