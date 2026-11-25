// S09-D8: the pending nonblocking updates of a process that ends commit.
// 9.6.2: the results of "Scheduled, but not executed, nonblocking assignments"
// "are not specified if the task is disabled"; 9.6.3 and 9.7 do not mention
// them.
// Decision: process::kill(), disable fork and disable of the block, task or
// fork that issued an update leave the update pending: it commits in the NBA
// region of its time slot, with or without a delay or an event control, and a
// plain ->> triggers its event. Nothing else of the process runs.
`timescale 1ns/1ns
interface bus;
  logic [7:0] d;
endinterface

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
  event e;
  int triggers;
  process p;

  always @(e) triggers++;
  always #4 clk = ~clk;

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
  endtask

  task issuer;
    issue();
    #1 after <= 1;
  endtask

  task automatic restore;
    v = 0; bits = 0; r = 0; st.a = 0; fa[1] = 0; fw = '{0, 0, 0}; pa[1] = 0; b.d = 0;
    v5 = 0; ve = 0; after = 0; triggers = 0;
  endtask

  task automatic show(string how);
    #2 $display("%-12s v=%0d bits=%h r=%.1f st=%0d fa=%0d fw=%0d%0d%0d pa=%h d=%0d e=%0d v5=%0d",
                how, v, bits, r, st.a, fa[1], fw[0], fw[1], fw[2], pa[1], b.d, triggers, v5);
    #30 $display("%-12s v5=%0d ve=%0d after=%0d", "", v5, ve, after);
    restore();
  endtask

  initial begin
    restore();
    #2;
    fork issuer; join
    show("returns");

    fork begin p = process::self(); issuer; end join_none
    #0 p.kill();
    show("kill");

    fork begin p = process::self(); issue(); p.kill(); after = 2; end join_none
    #0;
    show("kills itself");

    fork issuer; join_none
    #0 disable fork;
    show("disable fork");

    fork begin fork issuer; join_none #1; end join_none
    #0;
    #0 disable fork;
    show("grandchild");

    fork
      begin : blk issuer; end
      #0 disable blk;
    join
    show("block");

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

    begin : own
      issue();
      disable own;
      after = 3;
    end
    show("own block");
    $finish;
  end
endmodule
