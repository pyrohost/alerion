printf "containers: "
docker container prune --force
printf "volumes: "
docker volume prune --all --force
