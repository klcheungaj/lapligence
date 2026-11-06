// Records of the column form across the design's structure: a port connected
// to an element of an array of records (SV 23.3.3.3), a member array driving
// a net array and a dense array through a pattern (SV 10.9), a member
// initializer whose cells differ (SV 7.2.2), an automatic record reached by a
// mailbox and a queue (SV 15.4), `matches` on an element (SV 12.6), and writers
// of different bits of one member, which are separate storage (SV 9.2.2.2).
`ifndef N
`define N 16
`endif
typedef struct { int count; logic [7:0] data [`N]; } value_t;

module child(input value_t source, output value_t result);
  always_comb begin
    result = source;
    result.count = source.count + 1;
    result.data[1] = source.data[0] ^ source.data[1];
  end
endmodule

module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pk_t;
  typedef struct { logic [7:0] a; pk_t p; logic [7:0] arr [`N]; } in_t;
  typedef struct { in_t in [2]; logic [7:0] b; } out_t;
  typedef struct { byte c [`N] = '{default: 1, 3: 2}; int d = 5; } init_t;
  typedef struct { logic [3:0] n [`N]; logic [3:0] m; } whole_t;
  typedef struct { int a; logic [7:0] l [`N]; } msg_t;

  value_t source [2], result [2];
  child u (source[1], result[0]);

  out_t s;
  logic [7:0] x, y, z;
  logic clk = 0, en = 1;
  always_comb s.in[0].a = x;
  always_comb s.in[0].p.hi = y[3:0];
  always_latch if (en) s.in[0].p.lo = y[7:4];
  always_comb s.in[1].arr[1] = z;
  always_ff @(posedge clk) s.in[1].arr[2] <= x ^ z;
  always_comb s.b = s.in[0].a + s.in[1].arr[1];

  init_t ini, loc;

  wire [3:0] w [`N];
  wire [3:0] m;
  logic [3:0] dense [`N];
  logic [3:0] tail;
  whole_t src;
  assign '{w, m} = src;

  mailbox #(msg_t) box = new();
  msg_t qu [$];
  msg_t arr [3];
  int i;

  function automatic int first(init_t v);
    return v.c[3] + v.d;
  endfunction

  initial begin
    source[1].count = 7;
    source[1].data[0] = 8'h5a;
    source[1].data[1] = 8'ha5;
    #1 $display("port %0d %h %h", result[0].count, result[0].data[0], result[0].data[1]);
    source[1].data[0] = 8'h12;
    #1 $display("port %0d %h", result[0].count, result[0].data[1]);

    x = 8'h34;
    y = 8'h9c;
    z = 8'hf1;
    #1 $display("writers %h %h %h %h", s.in[0].a, s.in[0].p, s.in[1].arr[1], s.b);
    clk = 1;
    #1 $display("ff %h", s.in[1].arr[2]);

    $display("initializer %0d %0d %0d %0d", ini.c[0], ini.c[3], ini.c[4], ini.d);
    i = 3;
    ini.c[i] = 9;
    $display("after write %0d %0d %0d", ini.c[3], first(ini), first(loc));
    begin
      automatic init_t a;
      $display("automatic %0d %0d", a.c[2], a.c[3]);
    end

    for (i = 0; i < `N; i++) src.n[i] = 4'(unsigned'(i));
    src.m = 4'hb;
    #1 $display("net %h %h %h", w[0], w[3], m);
    '{dense, tail} = src;
    $display("dense %h %h %h", dense[1], dense[2], tail);

    begin
      automatic msg_t u;
      u.a = 9;
      i = 2;
      u.l[i] = 8'hab;
      box.put(u);
      qu.push_back(u);
      qu[0].a = 4;
      arr[1] = u;
    end
    begin
      automatic msg_t v;
      box.get(v);
      $display("mailbox %0d %h", v.a, v.l[2]);
      v = qu[0];
      $display("queue %0d %h", v.a, v.l[2]);
      i = 1;
      qu[0].l[i] = 8'hcd;
      $display("queue write %h", qu[0].l[1]);
      v = arr[1];
      $display("array %0d %h", v.a, v.l[2]);
      if (arr[1] matches '{a: 9, l: .*})
        $display("matches element");
    end
    $finish(0);
  end
endmodule
