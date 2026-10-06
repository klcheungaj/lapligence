// SIM-015: process handles as class properties and record members: copies of
// records, queue elements and class handles name the same process, defaults
// are null (SV 9.7, 7.2, 8.4, 6.8 Table 6-7).
class Holder;
  process p;
  process extra;
  function void grab();
    p = process::self();
  endfunction
  function string state();
    return p == null ? "null" : p.status().name();
  endfunction
endclass

module tb;
  typedef struct { process p; int tag; } rec_t;
  rec_t r, copy_r;
  rec_t rq[$];
  Holder h, alias_h;

  task automatic show_rec(input rec_t x);
    $display("rec %0d %s", x.tag, x.p.status().name());
  endtask

  initial begin
    rec_t local_r;
    h = new;
    alias_h = h;
    $display("defaults %0d %0d %s", r.p == null, h.extra == null, h.state());
    fork
      begin h.grab(); r.p = process::self(); r.tag = 7; #10; end
    join_none
    #1;
    copy_r = r;
    rq.push_back(r);
    local_r = rq[0];
    $display("same %0d %0d %0d %0d", alias_h.p == r.p, copy_r.p == r.p, rq[0].p == h.p,
             local_r.p == h.p);
    show_rec(copy_r);
    h.extra = local_r.p;
    alias_h.p.suspend();
    $display("suspended %s %s", r.p.status().name(), h.extra.status().name());
    rq[0].p.kill();
    $display("killed %s %s", h.state(), local_r.p.status().name());
    r.p = null;
    $display("cleared %0d %0d", r.p == null, copy_r.p != null);
    $finish(0);
  end
endmodule
