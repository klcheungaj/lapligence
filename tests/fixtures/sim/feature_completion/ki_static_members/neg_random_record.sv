// A static `rand` property of unpacked structure type is rejected (readme:
// "Random properties").
module tb;
  typedef struct {
    rand int a;
    int b;
  } rec_t;
  class st;
    static rand rec_t rec;
  endclass
  st o = new;
  initial if (o.randomize()) $display("%0d", st::rec.a);
endmodule
