// A record with a string member moves through native values: queues, class
// properties, mailboxes and function results. Rebuild with N = 8
// to take the same paths through per-cell leaves: the output is the same.
`ifndef N
`define N 16
`endif
module tb;
  typedef struct {
    string name;
    logic [7:0] a[`N];
    int s;
  } rec_t;

  class holder;
    rec_t r;
  endclass

  rec_t r, t, q[$];
  holder h = new;
  mailbox #(rec_t) box = new;
  int k = 3;

  function automatic rec_t make(string name, logic [7:0] v);
    make.name = name;
    make.a[1] = v;
    make.s = 7;
  endfunction

  initial begin
    r.name = "first";
    r.a[2] = 8'h20;
    r.a[k] = 8'h30;
    r.s = 5;

    q.push_back(r);
    r.a[2] = 8'h21;
    $display("%s %h %h %0d", q[0].name, q[0].a[2], q[0].a[k], q[0].s);
    t = q[0];
    $display("%s %h %h %h %0d", t.name, t.a[2], t.a[3], t.a[4], t.s);
    q[0].a[3] = 8'h31;
    t = q.pop_back();
    $display("%0d %h", q.size(), t.a[3]);

    h.r = r;
    h.r.a[3] = 8'h32;
    t = h.r;
    $display("%s %h %h", t.name, t.a[2], t.a[3]);

    box.put(r);
    box.get(t);
    $display("%s %h", t.name, t.a[2]);

    t = make("made", 8'h41);
    $display("%s %h %0d %s", t.name, t.a[1], t.s, make("direct", 8'h42).name);
    $finish(0);
  end
endmodule
