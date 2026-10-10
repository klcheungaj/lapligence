// SIM-026 A01: $fscanf from a checked-in text file (selected_destinations.txt)
// into bit, part and indexed-part selects, memory and multidimensional
// elements, packed and unpacked structure members and class properties
// (IEEE 1800-2009 21.3.4.3). Each line is one $fscanf; the last call meets
// end of file before any conversion and returns EOF.
module tb;
  typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
  } pair_t;
  typedef struct {
    int count;
    logic [7:0] tag;
    string name;
    real ratio;
  } rec_t;
  class Box;
    int k;
    logic [7:0] b;
    real r;
  endclass
  integer fd, c, i;
  reg [15:0] v;
  reg [7:0] mem[0:3];
  reg [7:0] grid[0:1][0:2];
  pair_t pair;
  rec_t rec;
  Box box;
  initial begin
    box = new;
    v = 16'h0000;
    pair = 16'h0000;
    i = 8;
    fd = $fopen("selected_destinations.txt", "r");
    c = $fscanf(fd, "%d %h %b %h %h %h", v[3:0], v[15:12], v[4], v[i+:4], mem[1], grid[1][2]);
    $display("A c=%0d v=%h mem1=%h grid12=%h", c, v, mem[1], grid[1][2]);
    c = $fscanf(fd, "%h %h %d %h %h", pair.hi, pair.lo[3:0], rec.count, rec.tag, box.b);
    $display("B c=%0d pair=%h count=%0d tag=%h b=%h", c, pair, rec.count, rec.tag, box.b);
    c = $fscanf(fd, "%s %f %f", rec.name, rec.ratio, box.r);
    $display("C c=%0d name=%s ratio=%f r=%f", c, rec.name, rec.ratio, box.r);
    c = $fscanf(fd, "%d", box.k);
    $display("D c=%0d v=%0d", c, box.k);
    $fclose(fd);
    $finish;
  end
endmodule
