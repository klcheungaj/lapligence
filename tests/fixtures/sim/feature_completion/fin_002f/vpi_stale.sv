// Handles used after their object ended (37.3.7, 37.16, 37.27, 37.42, 38.5).
`timescale 1ns/1ns
module tb;
  class node;
    int x = 3;
    node next;
  endclass
  int v = 7, dyn[], que[$];
  typedef struct {
    int a, b, c;
  } rec_t;
  rec_t recs[$];
  string words[string];
  node n;
  event done;
  logic [7:0] delayed;

  task automatic framed(input int k);
    begin : a_long_block_name
      int deep;
      deep = k;
      #1 $step("frame");
    end
  endtask

  initial begin
    dyn = '{10, 11, 12, 13};
    que = '{20, 21, 22};
    words["k"] = "w";
    n = new;
    n.next = new;
    $step("take");
    #1 dyn = new[2](dyn);
    que.delete(0);
    void'(que.pop_back());
    words.delete("k");
    n.next = null;
    n = null;
    #1 $step("gone");
    dyn = new[6](dyn);
    que = '{30, 31, 32, 33};
    words["k"] = "again";
    n = new;
    #1 $step("back");
    framed(5);
    #1 $step("left");
    fork : worker
      #100 $display("never");
    join_none
    #1 $step("thread");
    disable fork;
    #1 $step("ended");
    recs = '{'{1, 2, 3}};
    $step("grow");
    $step("pending");
    #1 $finish;
  end
  final $step("final");
endmodule
