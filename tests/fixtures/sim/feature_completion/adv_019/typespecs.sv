package p;
  typedef logic [3:0] nib_t;
endpackage

module tb;
  typedef enum bit [0:2] {red, yellow, blue} primary_colors;
  typedef primary_colors colors;
  typedef time my_time;
  typedef int count_t;
  typedef struct packed { int i1; bit bvec; } [1:3] parrtype;
  typedef parrtype [2:1] parrtype2;
  typedef parrtype2 unparrtype [6:4];
  typedef struct packed { logic [3:0] hi; bit [1:0][2:3] lo; } pk_t;
  typedef union packed { logic [7:0] w; pk_t p; } pu_t;
  typedef struct { struct { int a; } B; real r; p::nib_t n; } C;

  colors c;
  my_time t;
  count_t n;
  unparrtype arr [3:0];
  pu_t u;
  C cs;
  byte b;
  shortint si;
  longint li;
  integer ig;
  logic sc;
  real rl;
  parameter count_t P = 5;
  wire [3:0] w;

  initial begin
    $typespecs;
    $finish;
  end
endmodule
