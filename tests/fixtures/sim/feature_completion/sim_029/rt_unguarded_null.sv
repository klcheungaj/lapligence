// IEEE 1800-2009 18.5.12: an unguarded null dereference in a constraint is an
// evaluation error; randomize() fails and the run reports an error.
class node_c; int n; endclass
class list_c;
  rand int v;
  node_c next;
  constraint sort_c { v < next.n; }
endclass
module tb;
  initial begin
    static list_c l = new;
    int ok;
    l.v = 5;
    ok = l.randomize();
    $display("ok=%0d v=%0d", ok, l.v);
    $finish;
  end
endmodule
