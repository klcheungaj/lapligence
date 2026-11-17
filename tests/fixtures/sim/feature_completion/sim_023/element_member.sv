// SV 10.6.2, 6.4: a member of an unpacked structure element of a static
// unpacked array is a singular variable, so it is a force and release target
// (industry-standard behaviour). Writes of the other members, and of the
// whole element, leave the forced member; after the release it keeps its
// value until the next write. A run-time index inside the element is
// evaluated when the statement executes.
typedef struct { int f; int g; } s_t;
typedef struct { logic [7:0] tag; int arr[3]; } r_t;
module tb;
  s_t s[2];
  r_t e[2];
  int j = 2;
  initial begin
    s[0].f = 1; s[1].f = 2; s[1].g = 3;
    force s[1].f = 77;
    #1 $display("m1 %0d %0d", s[1].f, s[1].g);
    s[1].f = 5;
    s[1].g = 4;
    #1 $display("m2 %0d %0d", s[1].f, s[1].g);
    s[1] = '{f: 8, g: 9};
    #1 $display("m3 %0d %0d", s[1].f, s[1].g);
    release s[1].f;
    #1 $display("m4 %0d %0d", s[1].f, s[1].g);
    s[1].f = 6;
    #1 $display("m5 %0d %0d", s[1].f, s[1].g);
    e[1].tag = 8'h11;
    force e[1].arr[j] = 33;
    j = 0;
    e[1].arr[2] = 1;
    e[1].arr[0] = 2;
    e[1].tag = 8'h22;
    #1 $display("m6 %h %0d %0d %0d", e[1].tag, e[1].arr[0], e[1].arr[1], e[1].arr[2]);
    j = 2;
    release e[1].arr[j];
    e[1].arr[2] = 3;
    #1 $display("m7 %0d", e[1].arr[2]);
    $display("done");
    $finish;
  end
endmodule
