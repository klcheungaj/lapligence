// Every construct nested tens to hundreds of levels deep: far above the
// 60 to 70 levels that used to overflow the stack of an unoptimized llg, and
// shallow enough for every C compiler (nested statements become nested blocks).
`timescale 1ns/1ns

`define M0 1
`define M1 `M0 + 1
`define M2 `M1 + 1
`define M3 `M2 + 1
`define M4 `M3 + 1
`define M5 `M4 + 1
`define M6 `M5 + 1
`define M7 `M6 + 1
`define M8 `M7 + 1
`define M9 `M8 + 1
`define M10 `M9 + 1
`define M11 `M10 + 1
`define M12 `M11 + 1
`define M13 `M12 + 1
`define M14 `M13 + 1
`define M15 `M14 + 1
`define M16 `M15 + 1
`define M17 `M16 + 1
`define M18 `M17 + 1
`define M19 `M18 + 1
`define M20 `M19 + 1
`define M21 `M20 + 1
`define M22 `M21 + 1
`define M23 `M22 + 1
`define M24 `M23 + 1
`define M25 `M24 + 1
`define M26 `M25 + 1
`define M27 `M26 + 1
`define M28 `M27 + 1
`define M29 `M28 + 1
`define M30 `M29 + 1
`define M31 `M30 + 1
`define M32 `M31 + 1
`define M33 `M32 + 1
`define M34 `M33 + 1
`define M35 `M34 + 1
`define M36 `M35 + 1
`define M37 `M36 + 1
`define M38 `M37 + 1
`define M39 `M38 + 1
`define M40 `M39 + 1
`define M41 `M40 + 1
`define M42 `M41 + 1
`define M43 `M42 + 1
`define M44 `M43 + 1
`define M45 `M44 + 1
`define M46 `M45 + 1
`define M47 `M46 + 1
`define M48 `M47 + 1
`define M49 `M48 + 1
`define M50 `M49 + 1
`define M51 `M50 + 1
`define M52 `M51 + 1
`define M53 `M52 + 1
`define M54 `M53 + 1
`define M55 `M54 + 1
`define M56 `M55 + 1
`define M57 `M56 + 1
`define M58 `M57 + 1
`define M59 `M58 + 1
`define M60 `M59 + 1
`define M61 `M60 + 1
`define M62 `M61 + 1
`define M63 `M62 + 1
`define M64 `M63 + 1
`define M65 `M64 + 1
`define M66 `M65 + 1
`define M67 `M66 + 1
`define M68 `M67 + 1
`define M69 `M68 + 1
`define M70 `M69 + 1
`define M71 `M70 + 1
`define M72 `M71 + 1
`define M73 `M72 + 1
`define M74 `M73 + 1
`define M75 `M74 + 1
`define M76 `M75 + 1
`define M77 `M76 + 1
`define M78 `M77 + 1
`define M79 `M78 + 1
`define M80 `M79 + 1
`define M81 `M80 + 1
`define M82 `M81 + 1
`define M83 `M82 + 1
`define M84 `M83 + 1
`define M85 `M84 + 1
`define M86 `M85 + 1
`define M87 `M86 + 1
`define M88 `M87 + 1
`define M89 `M88 + 1
`define M90 `M89 + 1
`define M91 `M90 + 1
`define M92 `M91 + 1
`define M93 `M92 + 1
`define M94 `M93 + 1
`define M95 `M94 + 1
`define M96 `M95 + 1
`define M97 `M96 + 1
`define M98 `M97 + 1
`define M99 `M98 + 1
`define M100 `M99 + 1

module deeper #(parameter int LEVEL = 0) (input int d, output int q);
  if (LEVEL > 0) begin : more
    deeper #(LEVEL - 1) u (.d(d + 1), .q(q));
  end else begin : last
    assign q = d;
  end
endmodule

module tb;
  int a = 1;
  int y, deepest;
  logic b;
  string s = "x";
  logic clk = 1'b0;
  int index [256];
  int blocks = 0;

  function automatic int f(input int x);
    return x + 1;
  endfunction

  let l0 = a;
  let l1 = l0 + 1; let l2 = l1 + 1; let l3 = l2 + 1; let l4 = l3 + 1;
  let l5 = l4 + 1; let l6 = l5 + 1; let l7 = l6 + 1; let l8 = l7 + 1;
  let l9 = l8 + 1; let l10 = l9 + 1; let l11 = l10 + 1; let l12 = l11 + 1;
  let l13 = l12 + 1; let l14 = l13 + 1; let l15 = l14 + 1; let l16 = l15 + 1;
  let l17 = l16 + 1; let l18 = l17 + 1; let l19 = l18 + 1; let l20 = l19 + 1;
  let l21 = l20 + 1; let l22 = l21 + 1; let l23 = l22 + 1; let l24 = l23 + 1;
  let l25 = l24 + 1; let l26 = l25 + 1; let l27 = l26 + 1; let l28 = l27 + 1;
  let l29 = l28 + 1; let l30 = l29 + 1; let l31 = l30 + 1; let l32 = l31 + 1;
  let l33 = l32 + 1; let l34 = l33 + 1; let l35 = l34 + 1; let l36 = l35 + 1;
  let l37 = l36 + 1; let l38 = l37 + 1; let l39 = l38 + 1; let l40 = l39 + 1;
  let l41 = l40 + 1; let l42 = l41 + 1; let l43 = l42 + 1; let l44 = l43 + 1;
  let l45 = l44 + 1; let l46 = l45 + 1; let l47 = l46 + 1; let l48 = l47 + 1;
  let l49 = l48 + 1; let l50 = l49 + 1; let l51 = l50 + 1; let l52 = l51 + 1;
  let l53 = l52 + 1; let l54 = l53 + 1; let l55 = l54 + 1; let l56 = l55 + 1;
  let l57 = l56 + 1; let l58 = l57 + 1; let l59 = l58 + 1; let l60 = l59 + 1;

  class chain;
    int n = 0;
    function chain next();
      n++;
      return this;
    endfunction
  endclass

  class c0;
  class c1;
  class c2;
  class c3;
  class c4;
  class c5;
  class c6;
  class c7;
  class c8;
  class c9;
  class c10;
  class c11;
  class c12;
  class c13;
  class c14;
  class c15;
  class c16;
  class c17;
  class c18;
  class c19;
  class c20;
  class c21;
  class c22;
  class c23;
  class c24;
  class c25;
  class c26;
  class c27;
  class c28;
  class c29;
  class c30;
  class c31;
  class c32;
  class c33;
  class c34;
  class c35;
  class c36;
  class c37;
  class c38;
  class c39;
  class c40;
  class c41;
  class c42;
  class c43;
  class c44;
  class c45;
  class c46;
  class c47;
  class c48;
  class c49;
  class c50;
  class c51;
  class c52;
  class c53;
  class c54;
  class c55;
  class c56;
  class c57;
  class c58;
  class c59;
  int v = 60;
  endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass   endclass 
  c0::c1::c2::c3::c4::c5::c6::c7::c8::c9::c10::c11::c12::c13::c14::c15::c16::c17::c18::c19::c20::c21::c22::c23::c24::c25::c26::c27::c28::c29::c30::c31::c32::c33::c34::c35::c36::c37::c38::c39::c40::c41::c42::c43::c44::c45::c46::c47::c48::c49::c50::c51::c52::c53::c54::c55::c56::c57::c58::c59 inner;

  class implied;
    rand int x;
    constraint first { x > 0; }
    constraint rest {
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 -> x > 0 ->
      x == 5;
    }
  endclass

  // Generate blocks: if, case and loop nested in one another, an else chain.
  if (1) begin : i0 if (1) begin : i1 if (1) begin : i2 if (1) begin : i3 if (1) begin : i4 if
  (1) begin : i5 if (1) begin : i6 if (1) begin : i7 if (1) begin : i8 if (1) begin : i9 if (1)
  begin : i10 if (1) begin : i11 if (1) begin : i12 if (1) begin : i13 if (1) begin : i14 if (1)
  begin : i15 if (1) begin : i16 if (1) begin : i17 if (1) begin : i18 if (1) begin : i19 if (1)
  begin : i20 if (1) begin : i21 if (1) begin : i22 if (1) begin : i23 if (1) begin : i24 if (1)
  begin : i25 if (1) begin : i26 if (1) begin : i27 if (1) begin : i28 if (1) begin : i29 if (1)
  begin : i30 if (1) begin : i31 if (1) begin : i32 if (1) begin : i33 if (1) begin : i34 if (1)
  begin : i35 if (1) begin : i36 if (1) begin : i37 if (1) begin : i38 if (1) begin : i39 if (1)
  begin : i40 if (1) begin : i41 if (1) begin : i42 if (1) begin : i43 if (1) begin : i44 if (1)
  begin : i45 if (1) begin : i46 if (1) begin : i47 if (1) begin : i48 if (1) begin : i49 if (1)
  begin : i50 if (1) begin : i51 if (1) begin : i52 if (1) begin : i53 if (1) begin : i54 if (1)
  begin : i55 if (1) begin : i56 if (1) begin : i57 if (1) begin : i58 if (1) begin : i59 if (1)
  begin : i60 if (1) begin : i61 if (1) begin : i62 if (1) begin : i63 if (1) begin : i64 if (1)
  begin : i65 if (1) begin : i66 if (1) begin : i67 if (1) begin : i68 if (1) begin : i69 if (1)
  begin : i70 if (1) begin : i71 if (1) begin : i72 if (1) begin : i73 if (1) begin : i74 if (1)
  begin : i75 if (1) begin : i76 if (1) begin : i77 if (1) begin : i78 if (1) begin : i79 if (1)
  begin : i80 if (1) begin : i81 if (1) begin : i82 if (1) begin : i83 if (1) begin : i84 if (1)
  begin : i85 if (1) begin : i86 if (1) begin : i87 if (1) begin : i88 if (1) begin : i89 if (1)
  begin : i90 if (1) begin : i91 if (1) begin : i92 if (1) begin : i93 if (1) begin : i94 if (1)
  begin : i95 if (1) begin : i96 if (1) begin : i97 if (1) begin : i98 if (1) begin : i99
  initial blocks++; end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end
  case (1) 1: begin : s0 case (1) 1: begin : s1 case (1) 1: begin : s2 case (1) 1: begin : s3
  case (1) 1: begin : s4 case (1) 1: begin : s5 case (1) 1: begin : s6 case (1) 1: begin : s7
  case (1) 1: begin : s8 case (1) 1: begin : s9 case (1) 1: begin : s10 case (1) 1: begin : s11
  case (1) 1: begin : s12 case (1) 1: begin : s13 case (1) 1: begin : s14 case (1) 1: begin :
  s15 case (1) 1: begin : s16 case (1) 1: begin : s17 case (1) 1: begin : s18 case (1) 1: begin
  : s19 case (1) 1: begin : s20 case (1) 1: begin : s21 case (1) 1: begin : s22 case (1) 1:
  begin : s23 case (1) 1: begin : s24 case (1) 1: begin : s25 case (1) 1: begin : s26 case (1)
  1: begin : s27 case (1) 1: begin : s28 case (1) 1: begin : s29 case (1) 1: begin : s30 case
  (1) 1: begin : s31 case (1) 1: begin : s32 case (1) 1: begin : s33 case (1) 1: begin : s34
  case (1) 1: begin : s35 case (1) 1: begin : s36 case (1) 1: begin : s37 case (1) 1: begin :
  s38 case (1) 1: begin : s39 case (1) 1: begin : s40 case (1) 1: begin : s41 case (1) 1: begin
  : s42 case (1) 1: begin : s43 case (1) 1: begin : s44 case (1) 1: begin : s45 case (1) 1:
  begin : s46 case (1) 1: begin : s47 case (1) 1: begin : s48 case (1) 1: begin : s49 case (1)
  1: begin : s50 case (1) 1: begin : s51 case (1) 1: begin : s52 case (1) 1: begin : s53 case
  (1) 1: begin : s54 case (1) 1: begin : s55 case (1) 1: begin : s56 case (1) 1: begin : s57
  case (1) 1: begin : s58 case (1) 1: begin : s59 initial blocks++; end endcase end endcase end
  endcase end endcase end endcase end endcase end endcase end endcase end endcase end endcase
  end endcase end endcase end endcase end endcase end endcase end endcase end endcase end
  endcase end endcase end endcase end endcase end endcase end endcase end endcase end endcase
  end endcase end endcase end endcase end endcase end endcase end endcase end endcase end
  endcase end endcase end endcase end endcase end endcase end endcase end endcase end endcase
  end endcase end endcase end endcase end endcase end endcase end endcase end endcase end
  endcase end endcase end endcase end endcase end endcase end endcase end endcase end endcase
  end endcase end endcase end endcase end endcase end endcase
  for (genvar G0 = 0; G0 < 1; G0++) begin : l0_
  for (genvar G1 = 0; G1 < 1; G1++) begin : l1_
  for (genvar G2 = 0; G2 < 1; G2++) begin : l2_
  for (genvar G3 = 0; G3 < 1; G3++) begin : l3_
  for (genvar G4 = 0; G4 < 1; G4++) begin : l4_
  for (genvar G5 = 0; G5 < 1; G5++) begin : l5_
  for (genvar G6 = 0; G6 < 1; G6++) begin : l6_
  for (genvar G7 = 0; G7 < 1; G7++) begin : l7_
  for (genvar G8 = 0; G8 < 1; G8++) begin : l8_
  for (genvar G9 = 0; G9 < 1; G9++) begin : l9_
  for (genvar G10 = 0; G10 < 1; G10++) begin : l10_
  for (genvar G11 = 0; G11 < 1; G11++) begin : l11_
  for (genvar G12 = 0; G12 < 1; G12++) begin : l12_
  for (genvar G13 = 0; G13 < 1; G13++) begin : l13_
  for (genvar G14 = 0; G14 < 1; G14++) begin : l14_
  for (genvar G15 = 0; G15 < 1; G15++) begin : l15_
  for (genvar G16 = 0; G16 < 1; G16++) begin : l16_
  for (genvar G17 = 0; G17 < 1; G17++) begin : l17_
  for (genvar G18 = 0; G18 < 1; G18++) begin : l18_
  for (genvar G19 = 0; G19 < 1; G19++) begin : l19_
  for (genvar G20 = 0; G20 < 1; G20++) begin : l20_
  for (genvar G21 = 0; G21 < 1; G21++) begin : l21_
  for (genvar G22 = 0; G22 < 1; G22++) begin : l22_
  for (genvar G23 = 0; G23 < 1; G23++) begin : l23_
  for (genvar G24 = 0; G24 < 1; G24++) begin : l24_
  for (genvar G25 = 0; G25 < 1; G25++) begin : l25_
  for (genvar G26 = 0; G26 < 1; G26++) begin : l26_
  for (genvar G27 = 0; G27 < 1; G27++) begin : l27_
  for (genvar G28 = 0; G28 < 1; G28++) begin : l28_
  for (genvar G29 = 0; G29 < 1; G29++) begin : l29_
  for (genvar G30 = 0; G30 < 1; G30++) begin : l30_
  for (genvar G31 = 0; G31 < 1; G31++) begin : l31_
  for (genvar G32 = 0; G32 < 1; G32++) begin : l32_
  for (genvar G33 = 0; G33 < 1; G33++) begin : l33_
  for (genvar G34 = 0; G34 < 1; G34++) begin : l34_
  for (genvar G35 = 0; G35 < 1; G35++) begin : l35_
  for (genvar G36 = 0; G36 < 1; G36++) begin : l36_
  for (genvar G37 = 0; G37 < 1; G37++) begin : l37_
  for (genvar G38 = 0; G38 < 1; G38++) begin : l38_
  for (genvar G39 = 0; G39 < 1; G39++) begin : l39_
  for (genvar G40 = 0; G40 < 1; G40++) begin : l40_
  for (genvar G41 = 0; G41 < 1; G41++) begin : l41_
  for (genvar G42 = 0; G42 < 1; G42++) begin : l42_
  for (genvar G43 = 0; G43 < 1; G43++) begin : l43_
  for (genvar G44 = 0; G44 < 1; G44++) begin : l44_
  for (genvar G45 = 0; G45 < 1; G45++) begin : l45_
  for (genvar G46 = 0; G46 < 1; G46++) begin : l46_
  for (genvar G47 = 0; G47 < 1; G47++) begin : l47_
  for (genvar G48 = 0; G48 < 1; G48++) begin : l48_
  for (genvar G49 = 0; G49 < 1; G49++) begin : l49_
  for (genvar G50 = 0; G50 < 1; G50++) begin : l50_
  for (genvar G51 = 0; G51 < 1; G51++) begin : l51_
  for (genvar G52 = 0; G52 < 1; G52++) begin : l52_
  for (genvar G53 = 0; G53 < 1; G53++) begin : l53_
  for (genvar G54 = 0; G54 < 1; G54++) begin : l54_
  for (genvar G55 = 0; G55 < 1; G55++) begin : l55_
  for (genvar G56 = 0; G56 < 1; G56++) begin : l56_
  for (genvar G57 = 0; G57 < 1; G57++) begin : l57_
  for (genvar G58 = 0; G58 < 1; G58++) begin : l58_
  for (genvar G59 = 0; G59 < 1; G59++) begin : l59_
  initial blocks++;
  end end end end end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end end end end end end end end end end end end
  end end end end end end end end end end end end end end
  parameter int WHICH = 149;
  if (WHICH == 0) begin : e0 initial blocks += 0; end else
  if (WHICH == 1) begin : e1 initial blocks += 1; end else
  if (WHICH == 2) begin : e2 initial blocks += 2; end else
  if (WHICH == 3) begin : e3 initial blocks += 3; end else
  if (WHICH == 4) begin : e4 initial blocks += 4; end else
  if (WHICH == 5) begin : e5 initial blocks += 5; end else
  if (WHICH == 6) begin : e6 initial blocks += 6; end else
  if (WHICH == 7) begin : e7 initial blocks += 7; end else
  if (WHICH == 8) begin : e8 initial blocks += 8; end else
  if (WHICH == 9) begin : e9 initial blocks += 9; end else
  if (WHICH == 10) begin : e10 initial blocks += 10; end else
  if (WHICH == 11) begin : e11 initial blocks += 11; end else
  if (WHICH == 12) begin : e12 initial blocks += 12; end else
  if (WHICH == 13) begin : e13 initial blocks += 13; end else
  if (WHICH == 14) begin : e14 initial blocks += 14; end else
  if (WHICH == 15) begin : e15 initial blocks += 15; end else
  if (WHICH == 16) begin : e16 initial blocks += 16; end else
  if (WHICH == 17) begin : e17 initial blocks += 17; end else
  if (WHICH == 18) begin : e18 initial blocks += 18; end else
  if (WHICH == 19) begin : e19 initial blocks += 19; end else
  if (WHICH == 20) begin : e20 initial blocks += 20; end else
  if (WHICH == 21) begin : e21 initial blocks += 21; end else
  if (WHICH == 22) begin : e22 initial blocks += 22; end else
  if (WHICH == 23) begin : e23 initial blocks += 23; end else
  if (WHICH == 24) begin : e24 initial blocks += 24; end else
  if (WHICH == 25) begin : e25 initial blocks += 25; end else
  if (WHICH == 26) begin : e26 initial blocks += 26; end else
  if (WHICH == 27) begin : e27 initial blocks += 27; end else
  if (WHICH == 28) begin : e28 initial blocks += 28; end else
  if (WHICH == 29) begin : e29 initial blocks += 29; end else
  if (WHICH == 30) begin : e30 initial blocks += 30; end else
  if (WHICH == 31) begin : e31 initial blocks += 31; end else
  if (WHICH == 32) begin : e32 initial blocks += 32; end else
  if (WHICH == 33) begin : e33 initial blocks += 33; end else
  if (WHICH == 34) begin : e34 initial blocks += 34; end else
  if (WHICH == 35) begin : e35 initial blocks += 35; end else
  if (WHICH == 36) begin : e36 initial blocks += 36; end else
  if (WHICH == 37) begin : e37 initial blocks += 37; end else
  if (WHICH == 38) begin : e38 initial blocks += 38; end else
  if (WHICH == 39) begin : e39 initial blocks += 39; end else
  if (WHICH == 40) begin : e40 initial blocks += 40; end else
  if (WHICH == 41) begin : e41 initial blocks += 41; end else
  if (WHICH == 42) begin : e42 initial blocks += 42; end else
  if (WHICH == 43) begin : e43 initial blocks += 43; end else
  if (WHICH == 44) begin : e44 initial blocks += 44; end else
  if (WHICH == 45) begin : e45 initial blocks += 45; end else
  if (WHICH == 46) begin : e46 initial blocks += 46; end else
  if (WHICH == 47) begin : e47 initial blocks += 47; end else
  if (WHICH == 48) begin : e48 initial blocks += 48; end else
  if (WHICH == 49) begin : e49 initial blocks += 49; end else
  if (WHICH == 50) begin : e50 initial blocks += 50; end else
  if (WHICH == 51) begin : e51 initial blocks += 51; end else
  if (WHICH == 52) begin : e52 initial blocks += 52; end else
  if (WHICH == 53) begin : e53 initial blocks += 53; end else
  if (WHICH == 54) begin : e54 initial blocks += 54; end else
  if (WHICH == 55) begin : e55 initial blocks += 55; end else
  if (WHICH == 56) begin : e56 initial blocks += 56; end else
  if (WHICH == 57) begin : e57 initial blocks += 57; end else
  if (WHICH == 58) begin : e58 initial blocks += 58; end else
  if (WHICH == 59) begin : e59 initial blocks += 59; end else
  if (WHICH == 60) begin : e60 initial blocks += 60; end else
  if (WHICH == 61) begin : e61 initial blocks += 61; end else
  if (WHICH == 62) begin : e62 initial blocks += 62; end else
  if (WHICH == 63) begin : e63 initial blocks += 63; end else
  if (WHICH == 64) begin : e64 initial blocks += 64; end else
  if (WHICH == 65) begin : e65 initial blocks += 65; end else
  if (WHICH == 66) begin : e66 initial blocks += 66; end else
  if (WHICH == 67) begin : e67 initial blocks += 67; end else
  if (WHICH == 68) begin : e68 initial blocks += 68; end else
  if (WHICH == 69) begin : e69 initial blocks += 69; end else
  if (WHICH == 70) begin : e70 initial blocks += 70; end else
  if (WHICH == 71) begin : e71 initial blocks += 71; end else
  if (WHICH == 72) begin : e72 initial blocks += 72; end else
  if (WHICH == 73) begin : e73 initial blocks += 73; end else
  if (WHICH == 74) begin : e74 initial blocks += 74; end else
  if (WHICH == 75) begin : e75 initial blocks += 75; end else
  if (WHICH == 76) begin : e76 initial blocks += 76; end else
  if (WHICH == 77) begin : e77 initial blocks += 77; end else
  if (WHICH == 78) begin : e78 initial blocks += 78; end else
  if (WHICH == 79) begin : e79 initial blocks += 79; end else
  if (WHICH == 80) begin : e80 initial blocks += 80; end else
  if (WHICH == 81) begin : e81 initial blocks += 81; end else
  if (WHICH == 82) begin : e82 initial blocks += 82; end else
  if (WHICH == 83) begin : e83 initial blocks += 83; end else
  if (WHICH == 84) begin : e84 initial blocks += 84; end else
  if (WHICH == 85) begin : e85 initial blocks += 85; end else
  if (WHICH == 86) begin : e86 initial blocks += 86; end else
  if (WHICH == 87) begin : e87 initial blocks += 87; end else
  if (WHICH == 88) begin : e88 initial blocks += 88; end else
  if (WHICH == 89) begin : e89 initial blocks += 89; end else
  if (WHICH == 90) begin : e90 initial blocks += 90; end else
  if (WHICH == 91) begin : e91 initial blocks += 91; end else
  if (WHICH == 92) begin : e92 initial blocks += 92; end else
  if (WHICH == 93) begin : e93 initial blocks += 93; end else
  if (WHICH == 94) begin : e94 initial blocks += 94; end else
  if (WHICH == 95) begin : e95 initial blocks += 95; end else
  if (WHICH == 96) begin : e96 initial blocks += 96; end else
  if (WHICH == 97) begin : e97 initial blocks += 97; end else
  if (WHICH == 98) begin : e98 initial blocks += 98; end else
  if (WHICH == 99) begin : e99 initial blocks += 99; end else
  if (WHICH == 100) begin : e100 initial blocks += 100; end else
  if (WHICH == 101) begin : e101 initial blocks += 101; end else
  if (WHICH == 102) begin : e102 initial blocks += 102; end else
  if (WHICH == 103) begin : e103 initial blocks += 103; end else
  if (WHICH == 104) begin : e104 initial blocks += 104; end else
  if (WHICH == 105) begin : e105 initial blocks += 105; end else
  if (WHICH == 106) begin : e106 initial blocks += 106; end else
  if (WHICH == 107) begin : e107 initial blocks += 107; end else
  if (WHICH == 108) begin : e108 initial blocks += 108; end else
  if (WHICH == 109) begin : e109 initial blocks += 109; end else
  if (WHICH == 110) begin : e110 initial blocks += 110; end else
  if (WHICH == 111) begin : e111 initial blocks += 111; end else
  if (WHICH == 112) begin : e112 initial blocks += 112; end else
  if (WHICH == 113) begin : e113 initial blocks += 113; end else
  if (WHICH == 114) begin : e114 initial blocks += 114; end else
  if (WHICH == 115) begin : e115 initial blocks += 115; end else
  if (WHICH == 116) begin : e116 initial blocks += 116; end else
  if (WHICH == 117) begin : e117 initial blocks += 117; end else
  if (WHICH == 118) begin : e118 initial blocks += 118; end else
  if (WHICH == 119) begin : e119 initial blocks += 119; end else
  if (WHICH == 120) begin : e120 initial blocks += 120; end else
  if (WHICH == 121) begin : e121 initial blocks += 121; end else
  if (WHICH == 122) begin : e122 initial blocks += 122; end else
  if (WHICH == 123) begin : e123 initial blocks += 123; end else
  if (WHICH == 124) begin : e124 initial blocks += 124; end else
  if (WHICH == 125) begin : e125 initial blocks += 125; end else
  if (WHICH == 126) begin : e126 initial blocks += 126; end else
  if (WHICH == 127) begin : e127 initial blocks += 127; end else
  if (WHICH == 128) begin : e128 initial blocks += 128; end else
  if (WHICH == 129) begin : e129 initial blocks += 129; end else
  if (WHICH == 130) begin : e130 initial blocks += 130; end else
  if (WHICH == 131) begin : e131 initial blocks += 131; end else
  if (WHICH == 132) begin : e132 initial blocks += 132; end else
  if (WHICH == 133) begin : e133 initial blocks += 133; end else
  if (WHICH == 134) begin : e134 initial blocks += 134; end else
  if (WHICH == 135) begin : e135 initial blocks += 135; end else
  if (WHICH == 136) begin : e136 initial blocks += 136; end else
  if (WHICH == 137) begin : e137 initial blocks += 137; end else
  if (WHICH == 138) begin : e138 initial blocks += 138; end else
  if (WHICH == 139) begin : e139 initial blocks += 139; end else
  if (WHICH == 140) begin : e140 initial blocks += 140; end else
  if (WHICH == 141) begin : e141 initial blocks += 141; end else
  if (WHICH == 142) begin : e142 initial blocks += 142; end else
  if (WHICH == 143) begin : e143 initial blocks += 143; end else
  if (WHICH == 144) begin : e144 initial blocks += 144; end else
  if (WHICH == 145) begin : e145 initial blocks += 145; end else
  if (WHICH == 146) begin : e146 initial blocks += 146; end else
  if (WHICH == 147) begin : e147 initial blocks += 147; end else
  if (WHICH == 148) begin : e148 initial blocks += 148; end else
  if (WHICH == 149) begin : e149 initial blocks += 149; end else
  begin : e initial blocks = -1; end

  deeper #(100) instances (.d(a), .q(deepest));

  default clocking @(posedge clk); endclocking
  nested_not: assert property (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( not (
      not ( not ( not ( not ( not ( not ( not ( not ( not ( not ( a == 1 ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
  ) $display("property holds");
    else $display("property fails");

  chain links = new;
  implied picked = new;

  function automatic void arithmetic();
    y =
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a
      + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a + a +
      a + a + a + a + a;
    $display("binary %0d", y);
    y =
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( (
      ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( ( a ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) );
    $display("parentheses %0d", y);
    y =
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~( ~(
      ~( a ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) );
    $display("unary %h", y);
    y =
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f( f(
      0 ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) );
    $display("calls %0d", y);
    y =
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'( int'( byte'(
      int'( byte'( a + 300 ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) )
      ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) ) );
    $display("casts %0d", y);
    y =
      l60;
    $display("let %0d", y);
    y =
      `M100;
    $display("macro %0d", y);
  endfunction

  function automatic void selections();
    for (a = 0; a <= 100; a += 50) begin
      y =
        a == 0 ? 1000 : a == 1 ? 1001 : a == 2 ? 1002 : a == 3 ? 1003 : a == 4 ? 1004 : a == 5 ?
        1005 : a == 6 ? 1006 : a == 7 ? 1007 : a == 8 ? 1008 : a == 9 ? 1009 : a == 10 ? 1010 :
        a == 11 ? 1011 : a == 12 ? 1012 : a == 13 ? 1013 : a == 14 ? 1014 : a == 15 ? 1015 : a
        == 16 ? 1016 : a == 17 ? 1017 : a == 18 ? 1018 : a == 19 ? 1019 : a == 20 ? 1020 : a ==
        21 ? 1021 : a == 22 ? 1022 : a == 23 ? 1023 : a == 24 ? 1024 : a == 25 ? 1025 : a == 26
        ? 1026 : a == 27 ? 1027 : a == 28 ? 1028 : a == 29 ? 1029 : a == 30 ? 1030 : a == 31 ?
        1031 : a == 32 ? 1032 : a == 33 ? 1033 : a == 34 ? 1034 : a == 35 ? 1035 : a == 36 ?
        1036 : a == 37 ? 1037 : a == 38 ? 1038 : a == 39 ? 1039 : a == 40 ? 1040 : a == 41 ?
        1041 : a == 42 ? 1042 : a == 43 ? 1043 : a == 44 ? 1044 : a == 45 ? 1045 : a == 46 ?
        1046 : a == 47 ? 1047 : a == 48 ? 1048 : a == 49 ? 1049 : a == 50 ? 1050 : a == 51 ?
        1051 : a == 52 ? 1052 : a == 53 ? 1053 : a == 54 ? 1054 : a == 55 ? 1055 : a == 56 ?
        1056 : a == 57 ? 1057 : a == 58 ? 1058 : a == 59 ? 1059 : a == 60 ? 1060 : a == 61 ?
        1061 : a == 62 ? 1062 : a == 63 ? 1063 : a == 64 ? 1064 : a == 65 ? 1065 : a == 66 ?
        1066 : a == 67 ? 1067 : a == 68 ? 1068 : a == 69 ? 1069 : a == 70 ? 1070 : a == 71 ?
        1071 : a == 72 ? 1072 : a == 73 ? 1073 : a == 74 ? 1074 : a == 75 ? 1075 : a == 76 ?
        1076 : a == 77 ? 1077 : a == 78 ? 1078 : a == 79 ? 1079 : a == 80 ? 1080 : a == 81 ?
        1081 : a == 82 ? 1082 : a == 83 ? 1083 : a == 84 ? 1084 : a == 85 ? 1085 : a == 86 ?
        1086 : a == 87 ? 1087 : a == 88 ? 1088 : a == 89 ? 1089 : a == 90 ? 1090 : a == 91 ?
        1091 : a == 92 ? 1092 : a == 93 ? 1093 : a == 94 ? 1094 : a == 95 ? 1095 : a == 96 ?
        1096 : a == 97 ? 1097 : a == 98 ? 1098 : a == 99 ? 1099 : 7;
      $display("conditional %0d %0d", a, y);
    end
    for (a = 0; a <= 1; a++) begin
      b = a[0];
      b =
        (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b inside { (b
        inside { (b inside { (b inside { b }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) })
        }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) })
        }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) }) })
        }) }) }) });
      $display("inside %0d %0d", a, b);
    end
    a = 1;
    b =
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 && a > 0 &&
      a > 0;
    $display("logical %0d", b);
    y =
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[ index[
      index[ index[ 0 ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ]
      ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ];
    $display("index %0d", y);
    y =
      links.next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .next() .next() .next() .next() .next() .next() .next() .next() .next()
      .next() .next() .n;
    $display("members %0d", y);
  endfunction

  function automatic void concatenations();
    y =
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
      { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { a } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } };
    $display("concatenations %0d", y);
    y =
      int'({<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{
      {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{
      {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{
      {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{
      {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ {<<{ a }} }} }} }} }} }} }} }} }} }} }} }} }} }}
      }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }}
      }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }} }}
      }} }} }} }} }} }});
    $display("streams %0d", y);
    s =
      {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y",
      {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y",
      {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y",
      {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y",
      {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y", {"y",
      {"y", {"y", {"y", {"y", {"y", s } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
      } } } } } };
    $display("string %0d", s.len());
    { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { {
    { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { { y } } } } } } } } } } }
    } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } } }
    } } } } } } } } } } } } } } } } } } } } } } } = 12;
    $display("target %0d", y);
  endfunction

  function automatic void statements();
    begin begin begin begin begin begin begin begin begin begin begin begin begin begin begin
    begin begin begin begin begin begin begin begin begin begin begin begin begin begin begin
    begin begin begin begin begin begin begin begin begin begin begin begin begin begin begin
    begin begin begin begin begin begin begin begin begin begin begin begin begin begin begin a
    = 3; end end end end end end end end end end end end end end end end end end end end end end
    end end end end end end end end end end end end end end end end end end end end end end end
    end end end end end end end end end end end end end end end
    $display("begin %0d", a);
    if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if
    (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a >
    0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0)
    if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if
    (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a >
    0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0)
    if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if (a > 0) if
    (a > 0) if (a > 0) a = 4;
    $display("if %0d", a);
    if (a == 100) a = 0;
    else
    if (a == 101) a = 1;
    else
    if (a == 102) a = 2;
    else
    if (a == 103) a = 3;
    else
    if (a == 104) a = 4;
    else
    if (a == 105) a = 5;
    else
    if (a == 106) a = 6;
    else
    if (a == 107) a = 7;
    else
    if (a == 108) a = 8;
    else
    if (a == 109) a = 9;
    else
    if (a == 110) a = 10;
    else
    if (a == 111) a = 11;
    else
    if (a == 112) a = 12;
    else
    if (a == 113) a = 13;
    else
    if (a == 114) a = 14;
    else
    if (a == 115) a = 15;
    else
    if (a == 116) a = 16;
    else
    if (a == 117) a = 17;
    else
    if (a == 118) a = 18;
    else
    if (a == 119) a = 19;
    else
    if (a == 120) a = 20;
    else
    if (a == 121) a = 21;
    else
    if (a == 122) a = 22;
    else
    if (a == 123) a = 23;
    else
    if (a == 124) a = 24;
    else
    if (a == 125) a = 25;
    else
    if (a == 126) a = 26;
    else
    if (a == 127) a = 27;
    else
    if (a == 128) a = 28;
    else
    if (a == 129) a = 29;
    else
    if (a == 130) a = 30;
    else
    if (a == 131) a = 31;
    else
    if (a == 132) a = 32;
    else
    if (a == 133) a = 33;
    else
    if (a == 134) a = 34;
    else
    if (a == 135) a = 35;
    else
    if (a == 136) a = 36;
    else
    if (a == 137) a = 37;
    else
    if (a == 138) a = 38;
    else
    if (a == 139) a = 39;
    else
    if (a == 140) a = 40;
    else
    if (a == 141) a = 41;
    else
    if (a == 142) a = 42;
    else
    if (a == 143) a = 43;
    else
    if (a == 144) a = 44;
    else
    if (a == 145) a = 45;
    else
    if (a == 146) a = 46;
    else
    if (a == 147) a = 47;
    else
    if (a == 148) a = 48;
    else
    if (a == 149) a = 49;
    else
    if (a == 150) a = 50;
    else
    if (a == 151) a = 51;
    else
    if (a == 152) a = 52;
    else
    if (a == 153) a = 53;
    else
    if (a == 154) a = 54;
    else
    if (a == 155) a = 55;
    else
    if (a == 156) a = 56;
    else
    if (a == 157) a = 57;
    else
    if (a == 158) a = 58;
    else
    if (a == 159) a = 59;
    else
      a = 5;
    $display("else if %0d", a);
    for (int j0 = 0; j0 < 1; j0++) for (int j1 = 0; j1 < 1; j1++) for (int j2 = 0; j2 < 1; j2++)
    for (int j3 = 0; j3 < 1; j3++) for (int j4 = 0; j4 < 1; j4++) for (int j5 = 0; j5 < 1; j5++)
    for (int j6 = 0; j6 < 1; j6++) for (int j7 = 0; j7 < 1; j7++) for (int j8 = 0; j8 < 1; j8++)
    for (int j9 = 0; j9 < 1; j9++) a++;
    $display("for %0d", a);
    case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a)
    6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: case
    (a) 6: case (a) 6: case (a) 6: case (a) 6: case (a) 6: a = 7; default:; endcase default:;
    endcase default:; endcase default:; endcase default:; endcase default:; endcase default:;
    endcase default:; endcase default:; endcase default:; endcase default:; endcase default:;
    endcase default:; endcase default:; endcase default:; endcase default:; endcase default:;
    endcase default:; endcase default:; endcase default:; endcase
    $display("case %0d", a);
  endfunction

  initial begin
    foreach (index[i]) index[i] = (i + 1) % 256;
    #1;
    $display("blocks %0d", blocks);
    $display("instances %0d", deepest);
    arithmetic();
    selections();
    concatenations();
    statements();
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1; fork begin #1;
    fork begin #1; fork begin #1; fork begin #1; fork begin #1; a = 8; end join end join end
    join end join end join end join end join end join end join end join end join end join end
    join end join end join end join end join end join end join end join end join end join end
    join end join end join end join end join end join end join end join end join end join end
    join end join end join end join end join end join end join end join
    $display("fork %0d at %0t", a, $time);
    wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a
    == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8)
    wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a
    == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8)
    wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a
    == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8) wait (a == 8)
    wait (a == 8) a = 9;
    $display("wait %0d", a);
    #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1
    #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1
    #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1 #1
    #1 #1 #1 #1 #1 #1 #1 a = 10;
    $display("delay %0d at %0t", a, $time);
    inner = new;
    $display("class %0d", inner.v);
    if (!picked.randomize()) $display("randomize failed");
    $display("constraint %0d", picked.x);
    a = 1;
    #1 clk = 1'b1;
    #1 a = 0;
    clk = 1'b0;
    #1 clk = 1'b1;
    #1 $finish(0);
  end
endmodule
