// SIM-009-A03: legal (SV 9.4.2, 13.5.2), but a class handle property passed
// to a waiting handle `ref` formal has no marker the typed task can find, so
// the call is rejected explicitly instead of never waking.
module tb;
  class Item;
    int v;
  endclass
  class Holder;
    Item item;
  endclass

  task automatic wh(ref Item h);
    wait (h != null);
    $display("wh at %0d", $time);
  endtask

  Holder holder;
  initial begin
    holder = new;
    fork wh(holder.item); join_none
    #1 holder.item = new;
    #1 $finish;
  end
endmodule
