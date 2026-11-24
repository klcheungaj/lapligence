// Allocation failure matrix: dynamic arrays, queues and associative arrays,
// their growth, copies, methods and nested forms.
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int id;
    string tag;
    logic [99:0] wide;
  } item_t;

  int dynamic[], copy[];
  int queue[$], found[$];
  logic [199:0] wide_queue[$];
  string by_number[int];
  int by_name[string];
  logic [7:0] by_wide[logic [99:0]];
  int nested[][];
  int queues[string][$];
  item_t items[$];
  item_t one;
  byte stream[];
  int key;
  string name;

  initial begin
    dynamic = new[5];
    foreach (dynamic[i]) dynamic[i] = i * i;
    dynamic = new[12] (dynamic);
    copy = dynamic;
    copy[11] = 77;
    $display("dynamic %0d %0d %0d %0d", dynamic.size(), dynamic[4], copy[11], copy.sum());
    for (int i = 0; i < 40; i++) begin
      queue.push_back(40 - i);
      if (i % 3 == 0) queue.push_front(i);
      wide_queue.push_back(200'(unsigned'(i)) << 150);
    end
    queue.insert(7, 1000);
    queue.delete(2);
    found = queue.find with (item > 30);
    queue.sort();
    $display("queue %0d %0d %0d %0d %0d", queue.size(), queue[0], queue[$], found.size(), queue.pop_front());
    queue = queue[5:20];
    found = queue.unique();
    queue.reverse();
    $display("slice %0d %0d %0d %h", queue.size(), found.size(), queue[0], wide_queue[39][199:150]);
    for (int i = 0; i < 30; i++) begin
      by_number[i * 7] = $sformatf("n%0d", i);
      name = $sformatf("key%0d", i % 11);
      if (by_name.exists(name)) by_name[name] += i;
      else by_name[name] = i;
      by_wide[100'(unsigned'(i)) << 70] = 8'(unsigned'(i));
    end
    by_number.delete(14);
    $display("assoc %0d %0d %0d %s %0d %0d", by_number.num(), by_name.num(), by_wide.num(), by_number[21],
             by_name["key3"], by_wide.exists(100'd5 << 70));
    begin
      void'(by_number.last(key));
      void'(by_name.first(name));
      void'(by_name.next(name));
      $display("walk %0d %s", key, name);
    end
    nested = new[4];
    foreach (nested[i]) begin
      nested[i] = new[i + 2];
      for (int j = 0; j < i + 2; j++) nested[i][j] = i * 10 + j;
    end
    queues["a"].push_back(1);
    queues["b"].push_back(2);
    queues["a"].push_back(3);
    $display("nested %0d %0d %0d %0d", nested[3].size(), nested[2][3], queues["a"].size(), queues.num());
    for (int i = 0; i < 9; i++) begin
      one.id = i;
      one.tag = $sformatf("item%0d", i);
      one.wide = 100'(unsigned'(i)) << 90;
      items.push_back(one);
    end
    one = items[6];
    items.delete(1);
    $display("items %0d %s %0d %h", items.size(), one.tag, items[5].id, items[7].wide[99:90]);
    stream = {<<byte{dynamic}};
    copy = {>>{stream}};
    $display("stream %0d %0d %0d", stream.size(), copy.size(), copy[3]);
    dynamic.delete();
    by_name.delete();
    queue.delete();
    $display("empty %0d %0d %0d", dynamic.size(), by_name.num(), queue.size());
    $display("done");
    $finish;
  end
endmodule
