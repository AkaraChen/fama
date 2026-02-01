// Unformatted JSX file for testing fama
import React from "react";

function BadlyFormattedComponent({ title, items }) {
  return (
    <div className="container">
      <h1>{title}</h1>
      <ul>
        {items.map((item, index) => (
          <li key={index}>
            {item.name} - {item.description}
          </li>
        ))}
      </ul>
    </div>
  );
}

const App = () => {
  const [count, setCount] = React.useState(0);
  return (
    <div>
      <h1>Count:{count}</h1>
      <button onClick={() => setCount(count + 1)}>Increment</button>
      <BadlyFormattedComponent
        title="Test"
        items={[{ name: "Item 1", description: "Description 1" }]}
      />
    </div>
  );
};

export default App;
