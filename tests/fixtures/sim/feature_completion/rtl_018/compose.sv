// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_018/compose.sv
// RTL-018 A01 (SV 2009 §§23.11, 33.4, 33.7): a configuration selects library
// cells inside generate scopes, overrides parameters, and coexists with
// instance binds and an interface bind; an oversized fixed memory crosses the
// configured and bound instance ports through descriptor transport.
interface rtl018_mem_if;
  logic [7:0] mem [4];
  logic [7:0] total;
endinterface

interface rtl018_sum_if (input logic [7:0] m [4], output logic [7:0] s);
  assign s = m[0] + m[1] + m[2] + m[3];
endinterface

module rtl018_tap #(parameter int ID = 0) (
  input logic [7:0] v,
  input logic [8:0] image [131072]
);
  initial #(20 + ID) $display("tap %l ID=%0d v=%0d edge=%0d", ID, v, image[131071]);
endmodule

module tb;
  parameter int SCALE = 1;
  logic [7:0] x;
  logic [7:0] y [3];
  // 131072 nine-bit cells exceed the packed-value limit, so the array can
  // only cross ports as a descriptor.
  logic [8:0] big [131072];
  for (genvar g = 0; g < 2; g++) begin : rows
    rtl018_mem_if bus();
    rtl018_lane #(.GAIN(SCALE + g), .ID(g)) u(.x(x), .image(big), .y(y[g]));
  end
  if (SCALE > 0) begin : alt
    rtl018_lane #(.GAIN(SCALE), .ID(2)) u(.x(x), .image(big), .y(y[2]));
  end
  initial #5 $display("top %l SCALE=%0d", SCALE);
  initial begin
    big[0] = 9'd1;
    big[1] = 9'd2;
    big[131069] = 9'd5;
    big[131070] = 9'd3;
    big[131071] = 9'd4;
    x = 8'd2;
    for (int i = 0; i < 4; i++) begin
      rows[0].bus.mem[i] = 8'(i + 1);
      rows[1].bus.mem[i] = 8'(10 * (i + 1));
    end
    #1 $display("y=%0d,%0d,%0d totals=%0d,%0d", y[0], y[1], y[2],
                rows[0].bus.total, rows[1].bus.total);
    #30 $finish;
  end
endmodule

bind tb.rows[0].u rtl018_tap #(.ID(1)) tap(.v(y), .image(image));
bind tb.alt.u rtl018_tap #(.ID(2)) tap(.v(y), .image(image));
bind rtl018_mem_if rtl018_sum_if sum(.m(mem), .s(total));
