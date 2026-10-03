// SV2009 10.3, 9.4.2, 10.6.2, 13.4: continuous, evaluated-event and force
// evaluators call read-only helpers with private locals, loops (for, while,
// do-while, repeat, foreach), break/continue, early returns, case, private
// record/concatenation stores, named/default and const-ref arguments, and
// fixed-array/record operands. Equal results after operand changes are no
// events (9.4.2); edges use the least significant bit.
module tb;
  typedef logic [7:0] arr_t [0:3];
  typedef struct { logic [7:0] lo; logic [7:0] hi; } rec_t;
  arr_t a;
  rec_t r;
  bit [7:0] x;
  logic [7:0] c_ones, c_find, c_find9, c_nested, c_rep, c_max, f_arr, f_rec;

  function automatic logic [7:0] ones(input logic [7:0] v);
    logic [7:0] n = 0;
    do begin
      if (v[0]) n++;
      v >>= 1;
    end while (v != 0);
    return n;
  endfunction
  function automatic logic [7:0] find(input arr_t v, input logic [7:0] key = 8'd4);
    logic [7:0] idx = 8'hff;
    for (int i = 0; i < 4; i++) begin
      if (v[i] == 0) continue;
      if (v[i] >= key) begin
        idx = 8'(i);
        break;
      end
    end
    return idx;
  endfunction
  function automatic logic [7:0] cases(input logic [7:0] v);
    rec_t t;
    {t.hi, t.lo} = {v, ~v};
    case (v[1:0])
      2'd0: return t.lo;
      2'd1: return t.hi;
      default: return t.lo ^ t.hi;
    endcase
  endfunction
  function automatic logic [7:0] nested(input logic [7:0] v);
    return ones(v) + cases(v);
  endfunction
  function automatic logic [7:0] rep(input logic [7:0] v);
    logic [7:0] acc;
    acc = 0;
    repeat (v[2:0]) acc += 8'd2;
    return acc;
  endfunction
  function automatic logic [7:0] maxof(input arr_t v);
    logic [7:0] m;
    int i;
    m = v[0];
    i = 1;
    while (i < 4) begin
      if (v[i] > m) m = v[i];
      i++;
    end
    return m;
  endfunction
  function automatic logic [7:0] ends(const ref arr_t v);
    return v[0] + v[3];
  endfunction
  function automatic logic [7:0] rsum(input rec_t v);
    logic [7:0] s;
    s = 0;
    foreach (v.lo[i]) s[i] = v.lo[i] ^ v.hi[i];
    return s;
  endfunction

  assign c_ones = ones(x);
  assign c_find = find(a);
  assign c_find9 = find(.key(8'd9), .v(a));
  assign c_nested = nested(x);
  assign c_rep = rep(x);
  assign c_max = maxof(a);

  initial begin
    a = '{8'd0, 8'd5, 8'd9, 8'd2};
    r = '{lo: 8'h0f, hi: 8'h33};
    x = 8'h0b;
    force f_arr = maxof(a) + find(a, 8'd6);
    force f_rec = rsum(r);
    #1 $display("continuous %0d %0d %0d %0d %0d %0d", c_ones, c_find, c_find9, c_nested, c_rep, c_max);
    $display("force %0d %h", f_arr, f_rec);
    fork
      begin @(nested(x) or posedge ends(a)) $display("event_or %0t", $time); end
      begin @(find(.v(a), .key(8'd1))) $display("event_named %0t", $time); end
      begin @(maxof(a)) $display("event_max %0d %0t", maxof(a), $time); end
      begin
        #1 x = 8'h0c;
        #1 a[0] = 8'd1;
        #1 a[3] = 8'd7;
        #1 a[2] = 8'd30;
      end
    join
    r.hi = 8'hf0;
    #1 $display("continuous %0d %0d %0d %0d %0d %0d", c_ones, c_find, c_find9, c_nested, c_rep, c_max);
    $display("force %0d %h", f_arr, f_rec);
    release f_arr;
    a[1] = 8'd99;
    #1 $display("released %0d", f_arr);
    $finish(0);
  end
endmodule
