// SIM-010: fork branches of every join kind use the subroutine's by-value
// string, handle, real, event, record and container formals (SV 6.21, 9.3.2,
// 13.3, 13.5.1); outputs are copied out when the subroutine returns.
module tb;
  class C;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  typedef struct {
    string name;
    int    q[$];
  } rec_t;

  task automatic shs(input string s);
    $display("shs %s", s);
  endtask
  task automatic set(output string s, input string v);
    s = v;
  endtask
  task automatic show(input rec_t r);
    $display("show %s %0d", r.name, r.q.size());
  endtask

  // String inputs: read, passed on, written, nested forks.
  task automatic s_join(input string tag);
    fork
      $display("join %s", tag);
      begin
        #1 shs(tag);
      end
    join
    $display("join done %s", tag);
  endtask
  task automatic s_any(input string tag);
    fork
      $display("any %s", tag);
      begin
        #2 shs({tag, "-late"});
      end
    join_any
    $display("any done %s", tag);
  endtask
  task automatic s_write(input string tag);
    fork
      set(tag, "call");
      begin
        #1 tag = {tag, "+b0"};
      end
      begin
        #2 tag = {tag, "+b1"};
      end
    join
    $display("write done %s %0d", tag, tag.len());
  endtask
  task automatic s_nested(input string tag);
    fork
      begin
        fork
          $display("inner %s", tag);
          begin
            #1 tag = {tag, "!"};
          end
          begin
            #2 $display("detached %s", tag);
          end
        join_any
        $display("outer %s", tag);
      end
    join
    tag = "changed";
    $display("nested done %s", tag);
  endtask
  task automatic s_wait(input string tag);
    fork
      begin
        wait (tag == "go");
        $display("wait %s", tag);
      end
      begin
        #1 tag = "go";
      end
    join
  endtask

  // String, handle, real and process outputs; inout string and handle.
  task automatic outs(output string o, inout string io, output C h, inout C hio, output real r,
                      inout real rio, output process p);
    fork
      begin
        o = "o";
        h = new(4);
        p = process::self();
      end
      begin
        #1 io = {io, "+io"};
        hio = new(hio.v + 10);
        r = 1.5;
        rio = rio * 2.0;
      end
    join
    $display("outs in %s %s %0d %0d %0.2f %0.2f", o, io, h.v, hio.v, r, rio);
  endtask

  // Handle, chandle, real and event inputs.
  task automatic ins(input C h, input chandle ch, input real r, input event e);
    fork
      $display("ins %0d %0d %0.2f", h.v, ch == null, r);
      begin
        #1 ->e;
      end
      begin
        #2 h = new(7);
        r = r * 2.0;
      end
    join
    $display("ins done %0d %0.2f", h.v, r);
  endtask

  // Records with a container member, and container formals.
  task automatic r_join(input rec_t x, output rec_t o, inout rec_t io);
    fork
      show(x);
      begin
        #1 x.q.push_back(5);
        x.name = {x.name, "+"};
      end
      begin
        o.name = "out";
        o.q.push_back(1);
      end
      begin
        #1 io.q.push_back(9);
        io.name = {io.name, "+io"};
      end
    join
    $display("r_join %s %0d %0d", x.name, x.q.size(), x.q[$]);
  endtask
  task automatic r_none(input rec_t x, input int q[$]);
    fork
      begin
        #1 $display("r_none %s %0d %0d", x.name, x.q.size(), q.size());
      end
    join_none
    x.name = "changed";
    q.push_back(2);
  endtask
  task automatic r_nested(input rec_t x);
    fork
      fork
        begin
          #1 x.q.push_back(7);
        end
        $display("r_nested inner %s", x.name);
      join
    join
    $display("r_nested %0d", x.q.size());
  endtask

  // A static task's formals.
  task st(input string s, output string so, output C h, output rec_t r);
    fork
      $display("st %s", s);
      begin
        #1 so = {s, "-o"};
        h = new(8);
        r.name = "sr";
        r.q.push_back(3);
      end
    join
  endtask

  // A class method.
  class K;
    string nm = "k";
    task automatic m(input rec_t r, output string o, inout K peer);
      fork
        begin
          #1 r.q.push_back(2);
          o = {nm, r.name};
        end
        peer = this;
      join
      $display("m done %0d", r.q.size());
    endtask
  endclass

  string sout, sio, sto;
  C h0, h1, hst;
  real r0, r1;
  process p0;
  event ev;
  rec_t ro, rinout, rst;
  K k, kp;

  initial begin
    rec_t r;
    s_join("x");
    s_any("y");
    #5;
    s_write("w");
    s_nested("n");
    s_wait("s");
    #5;
    sio = "a";
    h1 = new(1);
    r1 = 2.0;
    outs(sout, sio, h0, h1, r0, r1, p0);
    $display("outs %s %s %0d %0d %0.2f %0.2f %0d", sout, sio, h0.v, h1.v, r0, r1, p0 != null);
    fork
      begin
        @ev $display("event");
      end
      ins(h1, null, 2.5, ev);
    join
    $display("caller %0d", h1.v);
    r.name = "rr";
    r.q.push_back(3);
    rinout.name = "ii";
    rinout.q.push_back(4);
    r_join(r, ro, rinout);
    $display("r_join out %s %0d %s %0d %0d %0d", ro.name, ro.q.size(), rinout.name, rinout.q.size(),
             rinout.q[1], r.q.size());
    r_none(r, r.q);
    #2;
    r_nested(r);
    st("t", sto, hst, rst);
    $display("st out %s %0d %s %0d", sto, hst.v, rst.name, rst.q[0]);
    k = new;
    k.m(r, sout, kp);
    $display("m out %s %s", sout, kp.nm);
    $finish;
  end
endmodule
