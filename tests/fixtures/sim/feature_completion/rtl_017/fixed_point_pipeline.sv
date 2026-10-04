// IEEE 1800-2009 11.11 in composition: Q8 fixed-point records (raw / 256)
// through continuous assignments, ports, nonblocking assignments, loops and
// overloads used inside another function's body.
typedef struct { int raw; } q8;

function automatic q8 qadd(q8 a, q8 b);
  q8 r;
  r.raw = a.raw + b.raw;
  return r;
endfunction

function automatic q8 qmul(q8 a, q8 b);
  q8 r;
  r.raw = (a.raw * b.raw) >>> 8;
  return r;
endfunction

function automatic q8 qfromr(real x);
  q8 r;
  r.raw = $rtoi(x * 256.0);
  return r;
endfunction

function automatic q8 qfromi(int i);
  q8 r;
  r.raw = i * 256;
  return r;
endfunction

bind + function q8 qadd(q8, q8);
bind * function q8 qmul(q8, q8);
bind = function q8 qfromr(real);
bind = function q8 qfromi(int);

function automatic q8 mac(q8 acc, q8 x, q8 c);
  return acc + x * c;
endfunction

module scale(input logic clk, input q8 x, output q8 y);
  q8 gain;
  initial gain = 1.5;
  always_ff @(posedge clk) y <= x * gain;
endmodule

module tb;
  logic clk = 0;
  q8 x, y, y2, s, acc;
  q8 taps [0:3];

  assign s = x + y;
  scale u(.clk(clk), .x(x), .y(y));
  scale u2(.clk(clk), .x(3), .y(y2));

  initial begin
    for (int i = 0; i < 4; i++) taps[i] = i + 1;
    x = 2;
    #1 clk = 1;
    #1 $display("y %0d s %0d y2 %0d", y.raw, s.raw, y2.raw);
    acc = 0;
    for (int i = 0; i < 4; i++) acc = mac(acc, taps[i], x);
    $display("acc %0d", acc.raw);
    x = 0.25;
    #1 clk = 0;
    #1 clk = 1;
    #1 $display("y %0d s %0d", y.raw, s.raw);
    $finish;
  end
endmodule
