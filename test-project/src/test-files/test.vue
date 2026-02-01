<template>
  <div class="container">
    <h1>{{ title }}</h1>
    <p>Count: {{ count }}</p>
    <ul>
      <li v-for="(item, index) in items" :key="index">
        {{ item.name }}: {{ item.value }}
      </li>
    </ul>
    <button @click="increment">Increment</button>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

interface Item {
  name: string;
  value: number;
}

const title = ref<string>('Test Vue Component');
const count = ref<number>(0);
const items = ref<Item[]>([
  { name: 'Item 1', value: 100 },
  { name: 'Item 2', value: 200 },
  { name: 'Item 3', value: 300 },
]);

function increment(): void {
  count.value++;
}
</script>

<style scoped>
.container {
  padding: 20px;
  background-color: #f5f5f5;
  border-radius: 8px;
}

h1 {
  color: #333;
  margin-bottom: 10px;
}

button {
  padding: 10px 20px;
  background-color: #42b883;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

button:hover {
  background-color: #33a06f;
}
</style>
