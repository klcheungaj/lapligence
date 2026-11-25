// Objects of every kind for the value formats of the VPI (38.15, 38.34).
`timescale 1ns/1ns
module tb;
  typedef enum {A, B} e_t;
  typedef struct packed {logic [3:0] h, l;} ps_t;
  class c_t;
    int f;
  endclass
  parameter P = 7;
  localparam LP = 2;
  parameter real RP = 2.5;
  reg [7:0] r8 = 8'h41;
  reg r1 = 1;
  wire [7:0] w8 = r8;
  wire w1 = r1;
  supply1 su;
  integer ig = 5;
  int i32 = -3;
  byte by = 65;
  bit [99:0] big = 100'h5;
  reg signed [7:0] s8 = -1;
  logic [0:7] up = 8'h81;
  time tm = 9;
  real re = 1.5;
  shortreal sr = 0.5;
  string st = "abc";
  e_t en = B;
  ps_t ps = 8'h3c;
  chandle ch;
  event ev;
  reg [3:0] mem[0:3];
  struct {int a; real b;} us;
  int dyn[], qq[$], as[int];
  c_t h;
  const int ci = 11;
  function int fn(int a);
    return a;
  endfunction
  task tk;
  endtask
  initial begin
    mem[0] = 1;
    mem[1] = 2;
    dyn = new[2];
    h = new;
  end
  initial #100 $finish;
endmodule
