// SIM-025: `$monitor` of members of an unpacked structure with a string
// member, held in a class property and in a static subroutine variable. The
// report follows the record: `evals` counts evaluations and its result joins
// the first list, so a line shows each evaluation. A store into the record
// evaluates the list; a store into other storage does not. The later lists
// print only when a listed value changed.
`timescale 1ns/1ns
module tb;
  typedef struct {
    int a;
    string s;
    real x;
  } rec_t;
  class C;
    rec_t r;
    int plain;
  endclass
  C h, other;
  int unrelated = 0;
  function int evals();
    static int n = 0;
    n = n + 1;
    return n;
  endfunction
  task keep(input int v);
    static rec_t r;
    r.a = v;
  endtask
  initial begin
    h = new;
    other = new;
    $monitor("%0t a=%0d s=%s e=%0d", $time, h.r.a, h.r.s, evals());
    #1 unrelated = 1;
    $strobe("%0t strobe a=%0d", $time, h.r.a);
    #1 h.plain = 1;
    #1 other.r.a = 9;
    #1 h.r.a = 1;
    #1 h.r.s = "s";
    #1 h.r.x = 1.5;
    #1 $monitor("%0t a=%0d s=%s", $time, h.r.a, h.r.s);
    #1 h.r.x = 2.5;
    #1 h.r.a = 2; h.r.a = 1;
    #1 h.r.a = 3;
    #1 h = other;
    #1 $monitor("%0t static a=%0d s=%s", $time, keep.r.a, keep.r.s);
    #1 keep(4);
    #1 keep.r.x = 0.5;
    #1 keep.r.s = "k";
    #1 $finish(0);
  end
endmodule
