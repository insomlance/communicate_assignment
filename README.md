# Communication Demo

Async communication process between servers or nodes by using a relayer based on tokio

## Run

You can direct run the test to see communication between A1 and B1 through relayer.
You can also input commands that predefined to create A2 B2 C1 D2 and then send message between them.

### Direct Run
In file ./biz/tests.rs, you can find test function called `test_func` and run it.
A1 and B1 will be registed on both client side and relayer side. Then A1 send message to B1, and B1 can receive it.

### Input Commands
There are three kinds of command, AddClient, SendMsg and Shutdown. Command format will be verified. 
But it is not strictly checked. Please input the command like the following shows, or the command may not work.

- `AddClient{A1;A;127.0.0.1:8787}` this command is used to register a node on both client side and relayer side.
The content of command in the '{}' are seperate by ';'. First item means the identity of current node. Sencond item
means the group of current node. And the third item is address it works on. 
Also, you can input AddClient{A2;A;127.0.0.1:8788} to register node A2 in A group, input AddClient{B1;B;127.0.0.1:9787} to
register node B1 in B group.
- `SendMsg{A1;B1;this is A1, to B group}` this command is used to send message from A1 to B1. The message will be sent to relayer first. Then relayer transfer it to B1. The content of command in the '{}' are seperate by ';'. First item means who
send the message and second item means who is the target. The third part is the message itself. SendMsg{A1;B2;this is A1, to B group} or SendMsg{A2;B1;this is A2, to B group} will not work when A2 and B2 are not registered. And the error information
will be told.
- `Shutdown` is used to close the whole service.

### Introduce More Groups(types)
`AddClient{C1;C;127.0.0.1:6787}` will automaticly register C1 of type C, and then C can send message or receive message.`SendMsg{C1;B1;this is C1, to B group}`.
If more servers of type D or type E are introduced in the future, code should be changed a little. I think whatever the type is, A B or C, the function of sending and receiving are same. That is to say MsgFromA and MsgFromB, or MsgToA and MsgToB seems similar. Their difference may be that each type has their own things to do that not mentioned in the task description.
So I reserve some place to add these function of different group.
In file ./custom/lib.rs, you can find function `receive_msg` and `send_msg`. The server of type D or type E should be added to the match branch and write the special code of this type at this place. There is no need to modify other file or code.

### Example
Copy the following command to the termial by sequence. The operation of register node may be a littel slow, because it will generate rsa pair.
```shell
AddClient{A1;A;127.0.0.1:8787}

SendMsg{A1;A1;this is A1, to A group}

AddClient{B1;B;127.0.0.1:9787}
AddClient{A2;A;127.0.0.1:8788}

SendMsg{A1;B1;this is A1, to B group}
SendMsg{A1;A2;this is A1, to A group}
SendMsg{B1;A2;this is B1, to A group}

AddClient{B2;B;127.0.0.1:9788}
AddClient{C1;C;127.0.0.1:7787}

SendMsg{A2;B2;this is A2, to B group}
SendMsg{B2;C1;this is B2, to C group}
``` 

## Design

### Roles
There are three roles, relayer, client(machine) and node(user). Node means different A1,A2,B1,B2,C1,D1... It's more like a logical concept. And client is real machine or something, a physical concept.You can launch all the nodes on the same machine. You can also launch one node on one machine.

### Concerns
- Async: All the base communication is async. The IO work of machine,node and relayer is non-blocking. Tokio is used to   implement the function. And channels are used to communicate between `spawn` and `async`. Tokio use the green thread to process the async tasks.
- Serde Serialize/Deserialize: Communication between machine and relayer should be serialized to transmit, and should be deserialize to get some necessary information.
- Rsa Authentication: When register the node, relayer will save the pubKey, and message from node will be verified whether the node has the correct identity.
- User working thread: Tokio's green thread is for IO task which is a frame part. When the frame part is finished, there may be some computation work of node like MsgToA, MsgToB. A simple thread pool is offered to hanle computation work. When a message is received, the following work will be automaticly processed by thread pool.

### Workflow
- launch relayer: After launch, relayer will listen to register request. Onece a register come, an async task for sending and receiving will be registered. Relayer will also save the information of each node.
- launch machine: After launch, machine will listen to register request. When a node wants to be work,it should send request to register on the machine.
- register node: Send requset to register both on machine and relayer. Then connection will be built between them.
- node send message: Node sign and send the message to the machine without knowing the relayer.
- relayer receive message: Relayer receive the message and parse it to know who is the destination.
- relayer send message: Relayer find the destination by route table and send to the destination.
- node receive message: Node receive the message and async transfer it to upper layer
- node do MsgToA: Node use the message to do something it like by using thread pool for user work.

### Code Structure
- frame
  The base async communication work. Listen for async task and process task.
  - relayer: relayer part
  - client: machine part
  - common: something is universal and utils like rsa
- custom
  User process part. Can change the structure of modify something if need.
  - base:
    It has machine mod and node mod. They process listen work and launch work of machine layer and node respectively. In most circumstances, it don't need to modify.
  - lib:
    It represents the difference of node and group. Accommodate and modify the code to satisfy different need. Most changes will be at here.
- biz
   For demonstration