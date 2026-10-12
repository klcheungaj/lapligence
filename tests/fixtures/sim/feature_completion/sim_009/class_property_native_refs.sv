// SIM-009-A03: class string and handle properties passed to waited `ref`
// formals (directly and forwarded through a recursive task) wake the tasks
// on the property's changes, whoever writes them (SV 9.4.2, 9.4.3, 13.5.2).
module tb;
  class Item;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  class Holder;
    string s;
    Item item;
  endclass

  task automatic word(ref string s, input int depth);
    if (depth > 0) word(s, depth - 1);
    else begin
      wait (s == "go");
      $display("word %s at %0d", s, $time);
      @(s);
      $display("word %s at %0d", s, $time);
    end
  endtask

  task automatic item(ref Item h, input int n);
    wait (h != null);
    $display("item %0d %0d at %0d", n, h.v, $time);
    if (n > 0) begin
      @(h);
      item(h, n - 1);
    end
  endtask

  Holder holder;
  initial begin
    holder = new;
    fork
      word(holder.s, 1);
      item(holder.item, 1);
    join_none
    #1 holder.s = "go";
    #1 holder.item = new(5);
    #1 holder.s = "stop";
    #1 holder.item.v = 6;
    #1 holder.item = new(7);
    #1 $finish;
  end
endmodule
