// SIM-025 A01: a static subroutine's string, chandle and process-handle
// variables are model storage (SV 6.21, 13.3.1), as its packed ones are, so a
// deferred report reads them when it prints (SV 21.2.2, 21.2.3): a later call
// or a later store in the same slot is what it shows. Only variables of
// automatic subroutines are barred (SV 13.3.2).
`timescale 1ns/1ns
module tb;
  class C;
    int v;
  endclass
  task static strs(input string s, input int k);
    $strobe("%0t s=%s k=%0d", $time, s, k);
  endtask
  task static store(input string s);
    string l;
    l = {s, "!"};
    $strobe("%0t l=%s s=%s", $time, l, s);
    s = "later";
    l = "L";
  endtask
  task static watch(input string s);
    $monitor("%0t watch s=%s", $time, s);
    #1 s = "b";
    #1 s = "b";
    #1 s = "c";
  endtask
  function static void chand(input chandle ch);
    $strobe("%0t null=%0d", $time, ch == null);
  endfunction
  task static obj(input C c);
    $strobe("%0t v=%0d", $time, c.v);
  endtask
  task static proc(input process p);
    $strobe("%0t status=%s", $time, p.status().name());
  endtask
  C o;
  process me;
  initial begin
    strs("x", 1);
    #1 store("c");
    #1 chand(null);
    #1 o = new;
    o.v = 1;
    obj(o);
    o.v = 2;
    #1 me = process::self();
    proc(me);
    #1 watch("a");
    #1 $finish(0);
  end
endmodule
