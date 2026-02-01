// Unformatted TypeScript file for testing fama
interface User {
  id: number;
  name: string;
  email: string;
  role?: "admin" | "user";
}

type Status = "active" | "inactive" | "pending";

function badlyFormattedFunction(a: number, b: number): number {
  const x = a + b;
  const y = x * 2;
  if (y > 100) {
    return y;
  } else {
    return x;
  }
}

class UserService {
  private users: User[] = [];
  addUser(user: User): void {
    this.users.push(user);
  }
  getUserById(id: number): User | undefined {
    return this.users.find((u) => u.id === id);
  }
}

const user: User = { id: 1, name: "Test User", email: "test@example.com" };

export { User, Status, badlyFormattedFunction, UserService, user };
