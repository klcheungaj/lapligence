// Updates of elements and members of static class properties and of built-in
// handles end with the process that issues them (readme: "Ended issuers").
`timescale 1ns / 1ns
module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } pair_t;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class node;
    int m;
    function new(int m);
      this.m = m;
    endfunction
  endclass
  class st;
    static int fa[3], f2[2][2];
    static logic [15:0] v;
    static pair_t pr, pa[2];
    static rec_t ra[2];
    static real rra[2];
    static node ha[2];
    static mailbox #(int) mb, mba[2];
    static semaphore sm;
    static process pp, ppa[2];
  endclass
  st o = new;
  node n1 = new(5);
  mailbox #(int) mb, mba[2], mb1 = new;
  semaphore sm, sma[2], sm1 = new;
  process pp, ppa[2];
  int plain, pl[2], x = 1;
  logic [7:0] w;
  task issue();
    st::fa[1] <= 4;
    o.fa[x+1] <= 6;
    st::f2[x][0] <= 5;
    st::f2[0] <= '{7, 8};
    st::fa[0][3:0] <= 9;
    st::v[x+:4] <= 4'ha;
    o.pr.hi <= 3;
    st::pa[1].lo <= 4;
    st::pa[x-1].hi[2] <= 1;
    st::ra[1] <= '{5, "y"};
    st::rra[x] <= 3.5;
    st::ha[x] <= n1;
    {w, st::pr.lo} <= 12'hfff;
    st::mb <= mb1;
    st::mba[x] <= mb1;
    st::sm <= sm1;
    st::pp <= process::self();
    st::ppa[x] <= process::self();
    mb <= mb1;
    mba[x] <= mb1;
    sm <= sm1;
    sma[x] <= sm1;
    pp <= process::self();
    ppa[x] <= process::self();
    plain <= 1;
    pl[x] <= 2;
  endtask
  task show(string step);
    $display("%s: %p %p %h %h %h %h", step, st::fa, st::f2, st::v, st::pr, st::pa[0], st::pa[1]);
    $display("  %0d %0.1f %b %h", st::ra[1].a, st::rra[1], st::ha[1] == null, w);
    $display("  %b%b%b%b%b %b%b%b%b%b%b", st::mb == null, st::mba[1] == null, st::sm == null,
             st::pp == null, st::ppa[1] == null, mb == null, mba[1] == null, sm == null,
             sma[1] == null, pp == null, ppa[1] == null);
    $display("  %0d %0d", plain, pl[1]);
  endtask
  initial begin
    st::v = 0;
    st::pr = 0;
    st::pa[0] = 0;
    st::pa[1] = 0;
    w = 0;
    #1;
    fork
      begin
        issue();
        #10;
      end
    join_none
    #0 disable fork;
    #1 show("disabled");
    fork
      begin
        issue();
        process::self().kill();
      end
    join_none
    #1 show("killed");
    issue();
    #1 show("kept");
    $finish(0);
  end
endmodule
