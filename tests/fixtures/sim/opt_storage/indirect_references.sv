// Each index or clock below is never written and is read by exactly one
// construct that names it outside a plain expression read. The unused-storage
// optimizer must keep its storage. A 2-state integral variable without an
// initializer holds '0 (IEEE 1800-2009 6.8, Table 6-7).
`timescale 1ns / 1ns
module tb;
  class Item;
    int id;
    function new(int v);
      id = v;
    endfunction
    function int show();
      return id;
    endfunction
  endclass

  event ev[2];
  event late;
  int i_trig;    // ->ev[i_trig]
  int i_flag;    // ev[i_flag].triggered
  int i_nb;      // ->>ev[i_nb]
  int i_arg;     // fire(ev[i_arg])
  int i_recv;    // items[i_recv].show()
  int i_handle;  // report(items[i_handle])
  bit clk;       // ->> @(posedge clk) late, and the clocked NBA below
  int never;
  int woke[3];
  bit late_fired;
  Item items[$];

  task automatic fire(event e);
    ->e;
  endtask

  function automatic int report(Item it);
    return it.id;
  endfunction

  // Waiters record when they wake; only the main process prints, so the
  // output does not depend on the order of processes within a time slot.
  initial begin
    @(ev[0]) woke[0] = $time;
    @(ev[0]) woke[1] = $time;
    @(ev[0]) woke[2] = $time;
  end

  initial begin
    @(late) late_fired = 1;
  end

  initial begin
    Item first;
    first = new(7);
    items.push_back(first);
    #1 ->ev[i_trig];
    $display("ev[0].triggered after -> = %0d", ev[i_flag].triggered);
    #1 ->>ev[i_nb];
    #1 fire(ev[i_arg]);
    $display("method call on item %0d", items[i_recv].show());
    $display("handle argument item %0d", report(items[i_handle]));
    ->> @(posedge clk) late;
    #1 $display("woke at %0d %0d %0d; late fired %0d; never = %0d",
                woke[0], woke[1], woke[2], late_fired, never);
    $finish;
  end

  // A separate process: the clocked nonblocking assignment is its only write.
  initial never <= @(posedge clk) 5;
endmodule
