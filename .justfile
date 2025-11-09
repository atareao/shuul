user     := "atareao"
name     := `basename ${PWD}`
version  := `vampus show`


list:
    @just --list

dev:
    cd frontend && pnpm i && pnpm run build && rm -rf ../backend/static && mkdir ../backend/static && cp -r ./dist/* ../backend/static
    cd backend && RUST_LOG=debug cargo run

[working-directory("./frontend")]
frontend:
    @pnpm run dev

[working-directory("./backend")]
backend:
    RUST_LOG=debug cargo run

build:
    @docker build \
        --tag={{user}}/{{name}}:{{version}} \
        --tag={{user}}/{{name}}:latest .

push:
    @docker image push --all-tags {{user}}/{{name}}

upgrade:
    #!/bin/fish
    vampus upgrade --patch
    set VERSION $(vampus show)
    cd backend
    cargo update
    cd ..
    git commit -am "Upgrade to version $VERSION"
    git tag -a "$VERSION" -m "Version $VERSION"
    # clean old docker images
    docker image list  | grep {{name}} | sort -r | tail -n +5 | awk '{print $3}' | while read id; echo $id; docker rmi $id; end
    just build push

[working-directory("./backend")]
revert:
    echo ${PWD}
    @sqlx migrate revert --target-version 0


