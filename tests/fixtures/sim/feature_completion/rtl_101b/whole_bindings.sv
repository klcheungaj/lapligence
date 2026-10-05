// RTL-101b: pattern variables bind whole values beyond packed capacity
// (records, tagged-union record members and member arrays).
module tb;
  typedef struct { logic [7:0] a [0:262143]; logic [3:0] k; } rec_t;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    rec_t s;
    bit [7:0] t;
  } tu_t;
  tu_t v;
  rec_t r, c;
  logic [1023:0] arr [0:2047];
  function automatic int f(input tu_t u);
    case (u) matches
      tagged w .q: return int'(q[2][7:0]);
      tagged s .z &&& z.k == 4'h5: return int'(z.a[3]) + 100;
      tagged s .z: return int'(z.k);
      default: return -1;
    endcase
  endfunction
  initial begin
    arr[2] = 1024'd7;
    v = tagged w arr;
    $display("A %0d", f(v));
    r.k = 4'h5;
    r.a[3] = 8'd9;
    v = tagged s r;
    $display("B %0d", f(v));
    r.k = 4'h2;
    v = tagged s r;
    $display("C %0d", f(v));
    if (v matches tagged s .y &&& y.k == 4'h2) $display("D %0d", y.a[3]);
    $display("E %0d", v matches tagged s .p ? p.k : 4'hf);
    if (r matches .x) begin
      c = x;
      r.a[3] = 8'd1;
      $display("F %h %0d %0d %0d", x.k, x.a[3], c.a[3], r.a[3]);
    end
    v = tagged t 8'd1;
    $display("G %0d", f(v));
    $finish(0);
  end
endmodule
