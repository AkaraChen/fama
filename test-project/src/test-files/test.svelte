<script lang="ts">
// Unformatted Svelte file for testing fama
export    let title:string='Test Svelte Component';
let    count:number=0;
const    items:Array<{name:string;value:number}>=[
    {name:"Item 1",value:100},
    {name:"Item 2",value:200},
    {name:"Item 3",value:300}
];

function    increment():void{
    count++;
}
</script>

<div    class="container">
    <h1>{title}</h1>
    <p>Count:{count}</p>
    <ul>
        {#each    items as item,index}
            <li>{item.name}:{item.value}</li>
        {/each}
    </ul>
    <button    on:click={increment}>Increment</button>
</div>

<style>
.container    {
    padding:20px;
    background-color:#f5f5f5;
    border-radius:8px;
}
h1    {
    color:#333;
    margin-bottom:10px;
}
button    {
    padding:10px 20px;
    background-color:#ff3e00;
    color:white;
    border:none;
    border-radius:4px;
    cursor:pointer;
}
button:hover    {
    background-color:#e63500;
}
</style>
