// A tagged union cannot be `rand` itself (18.4), but it can be a member of a
// `rand` packed structure: random bits then hold a declared tag and a legal
// value of the member it selects. The layout puts the tag above the members.
typedef enum bit [1:0] { R = 1, G = 2 } c_t;
typedef union tagged packed { bit [3:0] a; c_t b; void n; } u_t;
typedef struct packed { u_t t; bit p; } w_t;

class w_c;
  rand w_t w;
  rand w_t e[2];
endclass

module tb;
  initial begin
    static w_c o = new;
    int tags[4];
    bit [6:0] bits;
    int bad;
    repeat (300) begin
      if (o.randomize() != 1) $display("failed");
      bits = o.w;
      tags[bits[6:5]]++;
      if (bits[6:5] == 1 && !(bits[2:1] inside {1, 2})) bad++;
      bits = o.e[1];
      tags[bits[6:5]]++;
      if (bits[6:5] == 1 && !(bits[2:1] inside {1, 2})) bad++;
    end
    $display("bad=%0d", bad);
    foreach (tags[i]) if (tags[i] != 0) $display("tag %0d", i);
    $finish(0);
  end
endmodule
