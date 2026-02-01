// Unformatted TSX file for testing fama
import React from "react";

interface Props {
  title: string;
  count: number;
  items: Array<{ name: string; value: number }>;
}

function BadlyFormattedComponent({ title, count, items }: Props) {
  return (
    <div className="container">
      <h1>{title}</h1>
      <p>Count:{count}</p>
      <ul>
        {items.map((item, index) => (
          <li key={index}>
            {item.name}:{item.value}
          </li>
        ))}
      </ul>
    </div>
  );
}

const App: React.FC = () => {
  const [count, setCount] = React.useState<number>(0);
  const items: Array<{ name: string; value: number }> = [
    { name: "Item 1", value: 100 },
    { name: "Item 2", value: 200 },
  ];
  return (
    <div>
      <BadlyFormattedComponent title="Test TSX" count={count} items={items} />
      <button onClick={() => setCount(count + 1)}>Increment</button>
    </div>
  );
};

export default App;
