// S09-D8: what a process leaves behind that kill(), disable fork or disable
// ends.
// 9.6.2: the results of "Scheduled, but not executed, nonblocking assignments"
// "are not specified if the task is disabled"; 9.6.3 and 9.7 do not mention
// them, and 15.5.1 says nothing of a pending ->>.
// Decision: ordinary nonblocking assignments and plain ->> triggers stay
// pending and commit, with or without a delay or an event control. Timed
// triggers (->> #d, ->> @, also ->> #0) and the same-slot assignments to class
// handles and static class properties are dropped: they end with the process
// that issued them, like a process it has forked, so also after it has
// finished and by a disable of a block or task it has left. Wait fork does not
// wait for them.
`timescale 1ns/1ns
interface bus;
  logic [7:0] d;
endinterface

package pkg;
  class counter;
    static int total;
  endclass
endpackage

class item;
  static int count;
  static bit [99:0] wide;
  static real ratio;
  int tag;

  static task automatic set_count;
    count <= 4;
  endtask
endclass

class sized #(int N = 1);
  static int last;
endclass

module tb;
  typedef struct { int a; logic [3:0] f; } rec_t;
  logic clk = 0;
  bus b();
  virtual bus vb = b;
  logic [7:0] v, bits, v5, ve, after;
  real r;
  rec_t st;
  int fa[3], fw[3];
  logic [7:0] pa [1:0];
  event e, e0, e5, ec, er, en;
  int n, n0, n5, nc, nr, nn;
  process p;
  item made = new, h, ha[2], hw[2];
  item named = new;
  int x;

  always @(e) n++;
  always @(e0) n0++;
  always @(e5) n5++;
  always @(ec) nc++;
  always @(er) nr++;
  always @(en) nn++;
  always #4 clk = ~clk;

  function int bump(int by);
    x += by;
    return x;
  endfunction

  task automatic issue;
    v <= 1;
    bits <= #0 8'h3c;
    bits[7:6] <= 2'b11;
    r <= 1.5;
    st.a <= 3;
    fa[1] <= 7;
    fw <= '{1, 2, 3};
    pa[1][3:0] <= 4'ha;
    vb.d <= 13;
    ->> e;
    v5 <= #5 5;
    ve <= repeat (2) @(posedge clk) 6;

    ->> #0 e0;
    ->> #5 e5;
    ->> @(posedge clk) ec;
    ->> repeat (2) @(posedge clk) er;
    ->> @(bump(v)) en;
    h <= made;
    ha[1] <= made;
    hw <= '{made, made};
    item::set_count();
    named.wide <= '1;
    item::ratio <= 2.5;
    sized#(2)::last <= #0 9;
    pkg::counter::total <= 8;
  endtask

  task issuer;
    issue();
    #1 after <= 1;
  endtask

  task automatic restore;
    v = 0; bits = 0; r = 0; st.a = 0; fa[1] = 0; fw = '{0, 0, 0}; pa[1] = 0; b.d = 0;
    v5 = 0; ve = 0; after = 0; n = 0; n0 = 0; n5 = 0; nc = 0; nr = 0; nn = 0;
    h = null; ha[1] = null; hw = '{null, null};
    item::count = 0; item::wide = 0; item::ratio = 0; sized#(2)::last = 0;
    pkg::counter::total = 0;
  endtask

  task automatic show(string how);
    #1 $display("%-14s v=%0d bits=%h r=%.1f st=%0d fa=%0d fw=%0d%0d%0d pa=%h d=%0d e=%0d",
                how, v, bits, r, st.a, fa[1], fw[0], fw[1], fw[2], pa[1], b.d, n);
    #30 $display("%-14s v5=%0d ve=%0d after=%0d", "", v5, ve, after);
    $display("%-14s e0=%0d e5=%0d ec=%0d er=%0d en=%0d h=%0d%0d%0d static=%0d,%0d,%.1f,%0d,%0d",
             "", n0, n5, nc, nr, nn, h != null, ha[1] != null, hw[0] != null,
             item::count, item::wide[99], item::ratio, sized#(2)::last, pkg::counter::total);
    restore();
    #3;
  endtask

  initial begin
    restore();
    #2;
    fork issuer; join
    show("returns");

    fork begin p = process::self(); issuer; end join_none
    #0 p.kill();
    show("kill");

    fork begin p = process::self(); issuer; #100; end join_none
    #2 p.kill();
    show("kill, later");

    fork begin p = process::self(); issue(); p.kill(); after = 2; end join_none
    #0;
    show("kills itself");

    fork begin p = process::self(); issue(); end join_none
    #0 p.kill();
    show("finished");

    fork begin p = process::self(); issue(); end join_none
    #2 p.kill();
    show("finished, later");

    fork
      begin
        p = process::self();
        fork issue(); join_none
        #100;
      end
    join_none
    #0;
    #0 p.kill();
    show("parent");

    fork issuer; join_none
    #0 disable fork;
    show("disable fork");

    issue();
    #2 disable fork;
    show("own updates");

    fork begin fork issuer; join_none #1; end join_none
    #0;
    #0 disable fork;
    show("grandchild");

    fork
      begin : blk issuer; end
      #0 disable blk;
    join
    show("block");

    begin : left
      issue();
    end
    #0 disable left;
    show("left block");

    begin : left_later
      issue();
    end
    #2 disable left_later;
    show("left, later");

    begin : other
      after = 4;
    end
    issue();
    #2 disable other;
    show("other block");

    fork
      fork : fb issuer; join
      #0 disable fb;
    join
    show("fork block");

    fork
      issuer;
      #0 disable issuer;
    join
    show("task");

    issuer;
    #1 disable issuer;
    show("left task");

    begin : own
      issue();
      disable own;
      after = 3;
    end
    show("own block");

    issue();
    wait fork;
    show("wait fork");
    $finish;
  end
endmodule
