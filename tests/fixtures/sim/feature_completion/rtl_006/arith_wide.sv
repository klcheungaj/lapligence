// V2001 4.1.5-4.1.6; SV2009 11.4.3, 11.4.10, Table 11-4 at 8,128 and 8,129
// bits: 127 versus 128 limbs, where the compact backend's GMP multiply switches
// from row products to full mpn_mul_n. Operands follow arith_matrix.sv with the
// pattern replicated to W bits. Each result prints its low and high 64 bits
// and its count of one bits.
module wide #(parameter int W = 8128, parameter int SLOT = 0);
  localparam int N = 13;
  localparam logic [255:0] PAT =
      256'h9e3779b97f4a7c15_f39cc0605cedc834_1082276bf3a27251_f86c6a11d0c18e95;
  logic [W-1:0] v [0:N-1];
  logic [W-1:0] ua, ub;
  logic signed [W-1:0] sa, sb;

  function automatic string d(logic [W-1:0] r);
    return $sformatf("%h:%h:%0d", r[63:0], r[W-1-:64], $countones(r));
  endfunction

  initial begin
    v[0] = '0;
    v[1] = '0;
    v[1][0] = 1'b1;
    v[2] = v[1] + v[1];
    v[3] = v[2] + v[1];
    v[4] = '1;
    v[5] = v[1] << (W - 1);
    v[6] = v[5] - v[1];
    v[7] = v[5] + v[1];
    v[8] = {((W + 255) / 256){PAT}};
    v[9] = ~v[8];
    v[10] = v[4] - v[2];
    v[11] = 'x;
    v[12] = v[8];
    v[12][0] = 1'bz;
    #(SLOT);
    for (int i = 0; i < N; i++) begin
      ua = v[i];
      sa = v[i];
      $display("%0d %0d n %s", W, i, d(-sa));
      for (int j = 0; j < N; j++) begin
        ub = v[j];
        sb = v[j];
        $display("%0d %0d %0d s %s %s %s %s %s u %s %s %s %s %s m %s %s",
                 W, i, j,
                 d(sa + sb), d(sa - sb), d(sa * sb), d(sa / sb), d(sa % sb),
                 d(ua + ub), d(ua - ub), d(ua * ub), d(ua / ub), d(ua % ub),
                 d(sa / ub), d(sa % ub));
        if (i < 11)
          $display("%0d %0d %0d sh %s %s %s %s", W, i, j,
                   d(ua << ub), d(ua >> ub), d(sa >>> ub), d(sa >>> sb));
        // Odd bases other than 1 and -1 need one product per exponent bit;
        // keep those to small or negative exponents. Every other base
        // reaches 0, 1 or -1 and also takes the full-width exponents.
        if (!(i inside {3, 6, 7, 8, 10}) || j <= 3 || j >= 11)
          $display("%0d %0d %0d p %s %s", W, i, j, d(sa ** sb), d(ua ** ub));
        else if (!(j inside {6, 9}))
          $display("%0d %0d %0d p %s", W, i, j, d(sa ** sb));
      end
    end
    // One odd base with a full-width exponent: 3 ** (2**(W-1) - 1).
    $display("%0d odd %s", W, d(v[3] ** v[6]));
  end
endmodule

module tb;
  wide #(.W(8128), .SLOT(1)) w8128();
  wide #(.W(8129), .SLOT(2)) w8129();
  initial #3 $finish(0);
endmodule
