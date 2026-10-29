// Semantic-token requirements ST-76 and ST-77 (docs/semantic_tokens.md):
// names in port connection actuals keep their types whatever the instance
// connects them to, and an out-of-range constant select keeps its name's
// type. `pc_probe` is declared in port_connections_probe.sv, another project
// file, without the ports `bw_2`, `status`, `dur_0`, `sum`, `oob` and `rec_f`
// (Slang reports each as PortDoesNotExist and binds none of their actuals).
// `pc_missing` is declared nowhere (a black box).
module pc_top;
  localparam int N = 2;
  localparam int T = N * 2;
  typedef struct packed {
    logic [7:0] f;
  } rec_t;

  logic clk;
  reg          st_a [N];
  reg   [1:0]  st_b [N];
  logic [31:0] bw   [T];
  logic [31:0] dur  [N];
  logic [63:0] cnt  [T];
  rec_t        rec;

  // ST-76: a known definition; the ports after bw_1 do not exist, and the
  // second bw_0 repeats a connection.
  pc_probe probe (
      .clk  (clk),
      .bw_0 (bw[0]),
      .bw_1 (bw[1]),
      .bw_2 (bw[2]),
      .status({
        st_a[0],
        st_b[0],
        st_a[1],
        st_b[1]
      }),
      .dur_0(dur[0]),
      .sum  (cnt[0] + cnt[1] + cnt[2] + cnt[3]),
      .oob  (bw[7]),
      .rec_f(rec.f),
      .bw_0 (bw[3])
  );

  // ST-76: an unknown definition.
  pc_missing box (
      .status({st_a[1], st_b[1]}),
      .dur_1 (dur[1]),
      .total (cnt[1] + cnt[2]),
      .oob   (dur[5])
  );

  // ST-77: out-of-range constant selects outside connections.
  assign bw[6] = dur[4];
  always_comb cnt[9] = cnt[8] + T;
endmodule
