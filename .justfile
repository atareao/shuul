user     := "atareao"
name     := `basename ${PWD}`
version  := `git tag -l  | tail -n1`


list:
    @just --list

dev:
    cd frontend && pnpm i && pnpm run build && rm -rf ../backend/static && mkdir ../backend/static && cp -r ./dist/* ../backend/static
    cd backend && RUST_LOG=debug cargo run

[working-directory("./frontend")]
front:
    @pnpm run dev

[working-directory("./backend")]
back:
    RUST_LOG=debug cargo run

build:
    @docker build \
        --tag={{user}}/{{name}}:{{version}} \
        --tag={{user}}/{{name}}:latest .

push:
    @docker image push --all-tags {{user}}/{{name}}

[working-directory("./backend")]
revert:
    echo ${PWD}
    @sqlx migrate revert --target-version 0


