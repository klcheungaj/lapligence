// Nonblocking assignments to mailbox, semaphore and process handles and to
// elements and whole arrays of them (readme: "Kinds").
`timescale 1ns / 1ns
module tb;
  class st;
    static mailbox #(int) mb;
    static mailbox #(int) mba[2];
    static semaphore sm;
    static process pp, ppa[2];
  endclass
  mailbox #(int) mb, mb1 = new, mba[2];
  mailbox any, un1 = new(2);
  semaphore sm, sm1 = new(3), sma[2];
  process pp, pp1, ppa[2];
  event ev;
  bit x;
  int got;
  function automatic mailbox #(int) pick(int k);
    return k ? mb1 : null;
  endfunction
  task show(string step);
    $display("%0t %s: mb %b %b%b %b sm %b %b%b pp %b %b%b st %b %b%b %b %b %b%b", $time, step,
             mb == mb1, mba[0] == mb1, mba[1] == mb1, any == un1, sm == sm1, sma[0] == sm1,
             sma[1] == sm1, pp == pp1, ppa[0] == pp1, ppa[1] == pp1, st::mb == mb1,
             st::mba[0] == mb1, st::mba[1] == mb1, st::sm == sm1, st::pp == pp1, st::ppa[0] == pp1,
             st::ppa[1] == pp1);
  endtask
  initial begin
    pp1 = process::self();
    #1;
    mb <= mb1;
    any <= un1;
    sm <= sm1;
    pp <= process::self();
    mba[1] <= mb1;
    sma[~x] <= sm1;
    ppa[1] <= pp1;
    st::mb <= pick(1);
    st::mba[~x] <= mb1;
    st::sm <= sm1;
    st::pp <= pp1;
    st::ppa[0] <= pp1;
    show("issued");
    #0 show("still");
    #1 show("made");
    // The same objects.
    mb.put(5);
    void'(mb1.try_get(got));
    sm.get(2);
    $display("%0d %b %0d", got, sm1.try_get(2), pp.status() == process::RUNNING);
    mb <= null;
    any <= null;
    sm <= x ? sm1 : null;
    pp <= null;
    ppa[1] <= null;
    st::mb <= null;
    #1 show("cleared");
    // Values and elements are those at issue; the last update wins.
    pp <= pp1;
    pp <= null;
    mb <= null;
    mb <= mb1;
    mb1 = new;
    #1 $display("%b %b", pp == null, mb != null && mb != mb1);
    mb1 = mb;
    // Event controls.
    mb <= @(ev) null;
    pp <= repeat (2) @(ev) pp1;
    sm <= @(ev) sm1;
    st::pp <= @(ev) null;
    any <= @(ev) un1;
    mba[x] <= @(ev) mb1;
    sma[0] <= @(ev) sm1;
    ppa[0] <= @(ev) process::self();
    x = 1;
    #1 show("waiting");
    ->ev;
    #1 show("first");
    ->ev;
    #1 show("second");
    $finish(0);
  end
endmodule
