// RTL-101: structure patterns test the columns of a record beyond the
// dense-cell threshold.
module tb;
  typedef struct { logic [7:0] a [0:65536]; logic [3:0] tag; bit [7:0] n; } rec_t;
  rec_t r;
  initial begin
    r.tag = 4'h3;
    r.n = 8'd9;
    if (r matches '{a: .*, tag: 4'h3, n: .k}) $display("A %0d", k);
    else $display("A miss");
    if (r matches '{a: .*, tag: 4'h4, n: .*}) $display("B hit");
    else $display("B miss");
    if (r matches .*) $display("C any");
    case (r) matches
      '{a: .*, tag: 4'h2, n: .*}: $display("D two");
      '{a: .*, tag: 4'h3, n: .m} &&& (m == 8'd9): $display("D three %0d", m);
    endcase
    r.tag = 4'bx011;
    casex (r) matches
      '{a: .*, tag: 4'b0011, n: .*}: $display("E exact");
      default: $display("E default");
    endcase
    $finish(0);
  end
endmodule
