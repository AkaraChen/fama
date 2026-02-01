// Unformatted JavaScript file for testing fama
function badlyFormattedFunction(a, b, c) {
  const x = a + b;
  const y = c * 2;
  if (x > 10) {
    return y;
  } else {
    return x;
  }
}

const obj = { name: "test", value: 123 };

const array = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

class MyClass {
  constructor(name) {
    this.name = name;
  }
  getName() {
    return this.name;
  }
}

export { badlyFormattedFunction, obj, array, MyClass };
