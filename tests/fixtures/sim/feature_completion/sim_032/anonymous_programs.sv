// SIM-032: anonymous programs in a package and in the compilation unit
// declare programwide items in the enclosing namespace (IEEE 1800-2009 24.6).
package pk;
  int base = 100;
  program;
    function automatic int scaled(int x);
      return base + 2 * x;
    endfunction
    task automatic wait_and_say(int d);
      #d $display("package task t=%0d base=%0d", $time, base);
    endtask
    class Item;
      int v;
      function new(int x);
        v = x;
      endfunction
      function int get();
        return v + base;
      endfunction
    endclass
  endprogram
endpackage

program;
  function int twice(int x);
    return 2 * x;
  endfunction
  task automatic say(int x);
    $display("unit task %0d", x);
  endtask
endprogram

program p;
  import pk::*;
  initial begin
    static Item it = new(5);
    say(twice(21));
    $display("scaled=%0d item=%0d", scaled(3), it.get());
    wait_and_say(2);
  end
endprogram

module tb;
  p p0();
endmodule
