<?php

interface Repository {
    public function findAll(): array;
    public function findById(int $id): ?object;
}

trait Loggable {
    public function log(string $message): void {
        echo "[LOG] " . $message . "\n";
    }
}

class UserService implements Repository {
    use Loggable;

    private array $users = [];

    public function __construct(array $users) {
        $this->users = $users;
    }

    public function findAll(): array {
        return $this->users;
    }

    public function findById(int $id): ?object {
        return $this->users[$id] ?? null;
    }

    public function create(string $name): void {
        $this->users[] = $name;
    }
}

function processUsers(Repository $repo): array {
    return $repo->findAll();
}
