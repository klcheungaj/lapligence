// ADV-019 family 3: dynamic objects outside instance scopes (IEEE 1800-2009
// 37.17, 37.29, 37.10). The VPI application scopes.c finds the strings,
// class variables and queues a package, a named block and a static task
// declare, by name and through their scope. See readme.md for the expected
// output.
package dp;
  class PC;
    int x = 5;
  endclass
  PC pobj = new;
  string pname = "pk";
  int pq[$] = '{1, 2};
endpackage

module tb;
  class Node;
    int v;
    virtual bus_if vif;
    function new(int x);
      v = x;
    endfunction
  endclass
  bus_if bi();
  Node keep;
  task static remember();
    static Node held;
    held = keep;
    $scopes();
  endtask
  initial begin : blk
    static string label = "blk";
    keep = new(7);
    keep.vif = bi;
    remember();
    $display("%s %0d %s %0d", label, dp::pobj.x, dp::pname, dp::pq.size());
  end
endmodule

interface bus_if;
  logic [7:0] data;
endinterface
