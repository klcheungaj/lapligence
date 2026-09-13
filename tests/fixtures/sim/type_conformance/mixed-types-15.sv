module tb;
typedef enum logic signed [7:0] { FOUR_NEG=-3, FOUR_POS=5 } enum_four_t;
typedef enum bit [15:0] { TWO_ZERO=0, TWO_HIGH=32768 } enum_two_t;
typedef struct packed signed { logic [3:0] hi; logic [3:0] lo; } packed_struct_t;
typedef union packed { bit [15:0] whole; bit [1:0][7:0] bytes_; } packed_union_t;
typedef reg t_0; t_0 v_0;
typedef logic t_1; t_1 v_1;
typedef bit t_2; t_2 v_2;
typedef reg signed [7:0] t_3; t_3 v_3;
typedef logic [15:0] t_4; t_4 v_4;
typedef bit signed [7:0] t_5; t_5 v_5;
typedef byte t_6; t_6 v_6;
typedef byte unsigned t_7; t_7 v_7;
typedef shortint t_8; t_8 v_8;
typedef shortint unsigned t_9; t_9 v_9;
typedef int t_10; t_10 v_10;
typedef int unsigned t_11; t_11 v_11;
typedef longint t_12; t_12 v_12;
typedef longint unsigned t_13; t_13 v_13;
typedef integer t_14; t_14 v_14;
typedef integer unsigned t_15; t_15 v_15;
typedef time t_16; t_16 v_16;
typedef time signed t_17; t_17 v_17;
typedef logic signed [64:0] t_18; t_18 v_18;
typedef enum_four_t t_19; t_19 v_19;
typedef enum_two_t t_20; t_20 v_20;
typedef packed_struct_t t_21; t_21 v_21;
typedef packed_union_t t_22; t_22 v_22;
initial begin
begin t_15 a; t_0 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_0'(1'h1); end
1: begin a=t_15'(32'h80000005); b=t_0'(1'h0); end
2: begin a=t_15'(32'h0); b=t_0'(1'h1); end
endcase
$display("15/0/%0d/+=%b", iteration, a + b);
$display("15/0/%0d/-=%b", iteration, a - b);
$display("15/0/%0d/*=%b", iteration, a * b);
$display("15/0/%0d//=%b", iteration, a / b);
$display("15/0/%0d/mod=%b", iteration, a % b);
$display("15/0/%0d/&=%b", iteration, a & b);
$display("15/0/%0d/|=%b", iteration, a | b);
$display("15/0/%0d/^=%b", iteration, a ^ b);
$display("15/0/%0d/===%b", iteration, a == b);
$display("15/0/%0d/!==%b", iteration, a != b);
$display("15/0/%0d/====%b", iteration, a === b);
$display("15/0/%0d/!===%b", iteration, a !== b);
$display("15/0/%0d/<=%b", iteration, a < b);
$display("15/0/%0d/<==%b", iteration, a <= b);
$display("15/0/%0d/>=%b", iteration, a > b);
$display("15/0/%0d/>==%b", iteration, a >= b);
$display("15/0/%0d/&&=%b", iteration, a && b);
$display("15/0/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_1 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_1'(1'h1); end
1: begin a=t_15'(32'h80000005); b=t_1'(1'h0); end
2: begin a=t_15'(32'h0); b=t_1'(1'h1); end
endcase
$display("15/1/%0d/+=%b", iteration, a + b);
$display("15/1/%0d/-=%b", iteration, a - b);
$display("15/1/%0d/*=%b", iteration, a * b);
$display("15/1/%0d//=%b", iteration, a / b);
$display("15/1/%0d/mod=%b", iteration, a % b);
$display("15/1/%0d/&=%b", iteration, a & b);
$display("15/1/%0d/|=%b", iteration, a | b);
$display("15/1/%0d/^=%b", iteration, a ^ b);
$display("15/1/%0d/===%b", iteration, a == b);
$display("15/1/%0d/!==%b", iteration, a != b);
$display("15/1/%0d/====%b", iteration, a === b);
$display("15/1/%0d/!===%b", iteration, a !== b);
$display("15/1/%0d/<=%b", iteration, a < b);
$display("15/1/%0d/<==%b", iteration, a <= b);
$display("15/1/%0d/>=%b", iteration, a > b);
$display("15/1/%0d/>==%b", iteration, a >= b);
$display("15/1/%0d/&&=%b", iteration, a && b);
$display("15/1/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_2 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_2'(1'h1); end
1: begin a=t_15'(32'h80000005); b=t_2'(1'h0); end
2: begin a=t_15'(32'h0); b=t_2'(1'h1); end
endcase
$display("15/2/%0d/+=%b", iteration, a + b);
$display("15/2/%0d/-=%b", iteration, a - b);
$display("15/2/%0d/*=%b", iteration, a * b);
$display("15/2/%0d//=%b", iteration, a / b);
$display("15/2/%0d/mod=%b", iteration, a % b);
$display("15/2/%0d/&=%b", iteration, a & b);
$display("15/2/%0d/|=%b", iteration, a | b);
$display("15/2/%0d/^=%b", iteration, a ^ b);
$display("15/2/%0d/===%b", iteration, a == b);
$display("15/2/%0d/!==%b", iteration, a != b);
$display("15/2/%0d/====%b", iteration, a === b);
$display("15/2/%0d/!===%b", iteration, a !== b);
$display("15/2/%0d/<=%b", iteration, a < b);
$display("15/2/%0d/<==%b", iteration, a <= b);
$display("15/2/%0d/>=%b", iteration, a > b);
$display("15/2/%0d/>==%b", iteration, a >= b);
$display("15/2/%0d/&&=%b", iteration, a && b);
$display("15/2/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_3 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_3'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_3'(8'h0); end
2: begin a=t_15'(32'h0); b=t_3'(8'hfd); end
endcase
$display("15/3/%0d/+=%b", iteration, a + b);
$display("15/3/%0d/-=%b", iteration, a - b);
$display("15/3/%0d/*=%b", iteration, a * b);
$display("15/3/%0d//=%b", iteration, a / b);
$display("15/3/%0d/mod=%b", iteration, a % b);
$display("15/3/%0d/&=%b", iteration, a & b);
$display("15/3/%0d/|=%b", iteration, a | b);
$display("15/3/%0d/^=%b", iteration, a ^ b);
$display("15/3/%0d/===%b", iteration, a == b);
$display("15/3/%0d/!==%b", iteration, a != b);
$display("15/3/%0d/====%b", iteration, a === b);
$display("15/3/%0d/!===%b", iteration, a !== b);
$display("15/3/%0d/<=%b", iteration, a < b);
$display("15/3/%0d/<==%b", iteration, a <= b);
$display("15/3/%0d/>=%b", iteration, a > b);
$display("15/3/%0d/>==%b", iteration, a >= b);
$display("15/3/%0d/&&=%b", iteration, a && b);
$display("15/3/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_4 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_4'(16'h8005); end
1: begin a=t_15'(32'h80000005); b=t_4'(16'h0); end
2: begin a=t_15'(32'h0); b=t_4'(16'hfffd); end
endcase
$display("15/4/%0d/+=%b", iteration, a + b);
$display("15/4/%0d/-=%b", iteration, a - b);
$display("15/4/%0d/*=%b", iteration, a * b);
$display("15/4/%0d//=%b", iteration, a / b);
$display("15/4/%0d/mod=%b", iteration, a % b);
$display("15/4/%0d/&=%b", iteration, a & b);
$display("15/4/%0d/|=%b", iteration, a | b);
$display("15/4/%0d/^=%b", iteration, a ^ b);
$display("15/4/%0d/===%b", iteration, a == b);
$display("15/4/%0d/!==%b", iteration, a != b);
$display("15/4/%0d/====%b", iteration, a === b);
$display("15/4/%0d/!===%b", iteration, a !== b);
$display("15/4/%0d/<=%b", iteration, a < b);
$display("15/4/%0d/<==%b", iteration, a <= b);
$display("15/4/%0d/>=%b", iteration, a > b);
$display("15/4/%0d/>==%b", iteration, a >= b);
$display("15/4/%0d/&&=%b", iteration, a && b);
$display("15/4/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_5 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_5'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_5'(8'h0); end
2: begin a=t_15'(32'h0); b=t_5'(8'hfd); end
endcase
$display("15/5/%0d/+=%b", iteration, a + b);
$display("15/5/%0d/-=%b", iteration, a - b);
$display("15/5/%0d/*=%b", iteration, a * b);
$display("15/5/%0d//=%b", iteration, a / b);
$display("15/5/%0d/mod=%b", iteration, a % b);
$display("15/5/%0d/&=%b", iteration, a & b);
$display("15/5/%0d/|=%b", iteration, a | b);
$display("15/5/%0d/^=%b", iteration, a ^ b);
$display("15/5/%0d/===%b", iteration, a == b);
$display("15/5/%0d/!==%b", iteration, a != b);
$display("15/5/%0d/====%b", iteration, a === b);
$display("15/5/%0d/!===%b", iteration, a !== b);
$display("15/5/%0d/<=%b", iteration, a < b);
$display("15/5/%0d/<==%b", iteration, a <= b);
$display("15/5/%0d/>=%b", iteration, a > b);
$display("15/5/%0d/>==%b", iteration, a >= b);
$display("15/5/%0d/&&=%b", iteration, a && b);
$display("15/5/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_6 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_6'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_6'(8'h0); end
2: begin a=t_15'(32'h0); b=t_6'(8'hfd); end
endcase
$display("15/6/%0d/+=%b", iteration, a + b);
$display("15/6/%0d/-=%b", iteration, a - b);
$display("15/6/%0d/*=%b", iteration, a * b);
$display("15/6/%0d//=%b", iteration, a / b);
$display("15/6/%0d/mod=%b", iteration, a % b);
$display("15/6/%0d/&=%b", iteration, a & b);
$display("15/6/%0d/|=%b", iteration, a | b);
$display("15/6/%0d/^=%b", iteration, a ^ b);
$display("15/6/%0d/===%b", iteration, a == b);
$display("15/6/%0d/!==%b", iteration, a != b);
$display("15/6/%0d/====%b", iteration, a === b);
$display("15/6/%0d/!===%b", iteration, a !== b);
$display("15/6/%0d/<=%b", iteration, a < b);
$display("15/6/%0d/<==%b", iteration, a <= b);
$display("15/6/%0d/>=%b", iteration, a > b);
$display("15/6/%0d/>==%b", iteration, a >= b);
$display("15/6/%0d/&&=%b", iteration, a && b);
$display("15/6/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_7 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_7'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_7'(8'h0); end
2: begin a=t_15'(32'h0); b=t_7'(8'hfd); end
endcase
$display("15/7/%0d/+=%b", iteration, a + b);
$display("15/7/%0d/-=%b", iteration, a - b);
$display("15/7/%0d/*=%b", iteration, a * b);
$display("15/7/%0d//=%b", iteration, a / b);
$display("15/7/%0d/mod=%b", iteration, a % b);
$display("15/7/%0d/&=%b", iteration, a & b);
$display("15/7/%0d/|=%b", iteration, a | b);
$display("15/7/%0d/^=%b", iteration, a ^ b);
$display("15/7/%0d/===%b", iteration, a == b);
$display("15/7/%0d/!==%b", iteration, a != b);
$display("15/7/%0d/====%b", iteration, a === b);
$display("15/7/%0d/!===%b", iteration, a !== b);
$display("15/7/%0d/<=%b", iteration, a < b);
$display("15/7/%0d/<==%b", iteration, a <= b);
$display("15/7/%0d/>=%b", iteration, a > b);
$display("15/7/%0d/>==%b", iteration, a >= b);
$display("15/7/%0d/&&=%b", iteration, a && b);
$display("15/7/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_8 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_8'(16'h8005); end
1: begin a=t_15'(32'h80000005); b=t_8'(16'h0); end
2: begin a=t_15'(32'h0); b=t_8'(16'hfffd); end
endcase
$display("15/8/%0d/+=%b", iteration, a + b);
$display("15/8/%0d/-=%b", iteration, a - b);
$display("15/8/%0d/*=%b", iteration, a * b);
$display("15/8/%0d//=%b", iteration, a / b);
$display("15/8/%0d/mod=%b", iteration, a % b);
$display("15/8/%0d/&=%b", iteration, a & b);
$display("15/8/%0d/|=%b", iteration, a | b);
$display("15/8/%0d/^=%b", iteration, a ^ b);
$display("15/8/%0d/===%b", iteration, a == b);
$display("15/8/%0d/!==%b", iteration, a != b);
$display("15/8/%0d/====%b", iteration, a === b);
$display("15/8/%0d/!===%b", iteration, a !== b);
$display("15/8/%0d/<=%b", iteration, a < b);
$display("15/8/%0d/<==%b", iteration, a <= b);
$display("15/8/%0d/>=%b", iteration, a > b);
$display("15/8/%0d/>==%b", iteration, a >= b);
$display("15/8/%0d/&&=%b", iteration, a && b);
$display("15/8/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_9 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_9'(16'h8005); end
1: begin a=t_15'(32'h80000005); b=t_9'(16'h0); end
2: begin a=t_15'(32'h0); b=t_9'(16'hfffd); end
endcase
$display("15/9/%0d/+=%b", iteration, a + b);
$display("15/9/%0d/-=%b", iteration, a - b);
$display("15/9/%0d/*=%b", iteration, a * b);
$display("15/9/%0d//=%b", iteration, a / b);
$display("15/9/%0d/mod=%b", iteration, a % b);
$display("15/9/%0d/&=%b", iteration, a & b);
$display("15/9/%0d/|=%b", iteration, a | b);
$display("15/9/%0d/^=%b", iteration, a ^ b);
$display("15/9/%0d/===%b", iteration, a == b);
$display("15/9/%0d/!==%b", iteration, a != b);
$display("15/9/%0d/====%b", iteration, a === b);
$display("15/9/%0d/!===%b", iteration, a !== b);
$display("15/9/%0d/<=%b", iteration, a < b);
$display("15/9/%0d/<==%b", iteration, a <= b);
$display("15/9/%0d/>=%b", iteration, a > b);
$display("15/9/%0d/>==%b", iteration, a >= b);
$display("15/9/%0d/&&=%b", iteration, a && b);
$display("15/9/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_10 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_10'(32'h80000005); end
1: begin a=t_15'(32'h80000005); b=t_10'(32'h0); end
2: begin a=t_15'(32'h0); b=t_10'(32'hfffffffd); end
endcase
$display("15/10/%0d/+=%b", iteration, a + b);
$display("15/10/%0d/-=%b", iteration, a - b);
$display("15/10/%0d/*=%b", iteration, a * b);
$display("15/10/%0d//=%b", iteration, a / b);
$display("15/10/%0d/mod=%b", iteration, a % b);
$display("15/10/%0d/&=%b", iteration, a & b);
$display("15/10/%0d/|=%b", iteration, a | b);
$display("15/10/%0d/^=%b", iteration, a ^ b);
$display("15/10/%0d/===%b", iteration, a == b);
$display("15/10/%0d/!==%b", iteration, a != b);
$display("15/10/%0d/====%b", iteration, a === b);
$display("15/10/%0d/!===%b", iteration, a !== b);
$display("15/10/%0d/<=%b", iteration, a < b);
$display("15/10/%0d/<==%b", iteration, a <= b);
$display("15/10/%0d/>=%b", iteration, a > b);
$display("15/10/%0d/>==%b", iteration, a >= b);
$display("15/10/%0d/&&=%b", iteration, a && b);
$display("15/10/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_11 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_11'(32'h80000005); end
1: begin a=t_15'(32'h80000005); b=t_11'(32'h0); end
2: begin a=t_15'(32'h0); b=t_11'(32'hfffffffd); end
endcase
$display("15/11/%0d/+=%b", iteration, a + b);
$display("15/11/%0d/-=%b", iteration, a - b);
$display("15/11/%0d/*=%b", iteration, a * b);
$display("15/11/%0d//=%b", iteration, a / b);
$display("15/11/%0d/mod=%b", iteration, a % b);
$display("15/11/%0d/&=%b", iteration, a & b);
$display("15/11/%0d/|=%b", iteration, a | b);
$display("15/11/%0d/^=%b", iteration, a ^ b);
$display("15/11/%0d/===%b", iteration, a == b);
$display("15/11/%0d/!==%b", iteration, a != b);
$display("15/11/%0d/====%b", iteration, a === b);
$display("15/11/%0d/!===%b", iteration, a !== b);
$display("15/11/%0d/<=%b", iteration, a < b);
$display("15/11/%0d/<==%b", iteration, a <= b);
$display("15/11/%0d/>=%b", iteration, a > b);
$display("15/11/%0d/>==%b", iteration, a >= b);
$display("15/11/%0d/&&=%b", iteration, a && b);
$display("15/11/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_12 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_12'(64'h8000000000000005); end
1: begin a=t_15'(32'h80000005); b=t_12'(64'h0); end
2: begin a=t_15'(32'h0); b=t_12'(64'hfffffffffffffffd); end
endcase
$display("15/12/%0d/+=%b", iteration, a + b);
$display("15/12/%0d/-=%b", iteration, a - b);
$display("15/12/%0d/*=%b", iteration, a * b);
$display("15/12/%0d//=%b", iteration, a / b);
$display("15/12/%0d/mod=%b", iteration, a % b);
$display("15/12/%0d/&=%b", iteration, a & b);
$display("15/12/%0d/|=%b", iteration, a | b);
$display("15/12/%0d/^=%b", iteration, a ^ b);
$display("15/12/%0d/===%b", iteration, a == b);
$display("15/12/%0d/!==%b", iteration, a != b);
$display("15/12/%0d/====%b", iteration, a === b);
$display("15/12/%0d/!===%b", iteration, a !== b);
$display("15/12/%0d/<=%b", iteration, a < b);
$display("15/12/%0d/<==%b", iteration, a <= b);
$display("15/12/%0d/>=%b", iteration, a > b);
$display("15/12/%0d/>==%b", iteration, a >= b);
$display("15/12/%0d/&&=%b", iteration, a && b);
$display("15/12/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_13 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_13'(64'h8000000000000005); end
1: begin a=t_15'(32'h80000005); b=t_13'(64'h0); end
2: begin a=t_15'(32'h0); b=t_13'(64'hfffffffffffffffd); end
endcase
$display("15/13/%0d/+=%b", iteration, a + b);
$display("15/13/%0d/-=%b", iteration, a - b);
$display("15/13/%0d/*=%b", iteration, a * b);
$display("15/13/%0d//=%b", iteration, a / b);
$display("15/13/%0d/mod=%b", iteration, a % b);
$display("15/13/%0d/&=%b", iteration, a & b);
$display("15/13/%0d/|=%b", iteration, a | b);
$display("15/13/%0d/^=%b", iteration, a ^ b);
$display("15/13/%0d/===%b", iteration, a == b);
$display("15/13/%0d/!==%b", iteration, a != b);
$display("15/13/%0d/====%b", iteration, a === b);
$display("15/13/%0d/!===%b", iteration, a !== b);
$display("15/13/%0d/<=%b", iteration, a < b);
$display("15/13/%0d/<==%b", iteration, a <= b);
$display("15/13/%0d/>=%b", iteration, a > b);
$display("15/13/%0d/>==%b", iteration, a >= b);
$display("15/13/%0d/&&=%b", iteration, a && b);
$display("15/13/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_14 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_14'(32'h80000005); end
1: begin a=t_15'(32'h80000005); b=t_14'(32'h0); end
2: begin a=t_15'(32'h0); b=t_14'(32'hfffffffd); end
endcase
$display("15/14/%0d/+=%b", iteration, a + b);
$display("15/14/%0d/-=%b", iteration, a - b);
$display("15/14/%0d/*=%b", iteration, a * b);
$display("15/14/%0d//=%b", iteration, a / b);
$display("15/14/%0d/mod=%b", iteration, a % b);
$display("15/14/%0d/&=%b", iteration, a & b);
$display("15/14/%0d/|=%b", iteration, a | b);
$display("15/14/%0d/^=%b", iteration, a ^ b);
$display("15/14/%0d/===%b", iteration, a == b);
$display("15/14/%0d/!==%b", iteration, a != b);
$display("15/14/%0d/====%b", iteration, a === b);
$display("15/14/%0d/!===%b", iteration, a !== b);
$display("15/14/%0d/<=%b", iteration, a < b);
$display("15/14/%0d/<==%b", iteration, a <= b);
$display("15/14/%0d/>=%b", iteration, a > b);
$display("15/14/%0d/>==%b", iteration, a >= b);
$display("15/14/%0d/&&=%b", iteration, a && b);
$display("15/14/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_15 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_15'(32'h80000005); end
1: begin a=t_15'(32'h80000005); b=t_15'(32'h0); end
2: begin a=t_15'(32'h0); b=t_15'(32'hfffffffd); end
endcase
$display("15/15/%0d/+=%b", iteration, a + b);
$display("15/15/%0d/-=%b", iteration, a - b);
$display("15/15/%0d/*=%b", iteration, a * b);
$display("15/15/%0d//=%b", iteration, a / b);
$display("15/15/%0d/mod=%b", iteration, a % b);
$display("15/15/%0d/&=%b", iteration, a & b);
$display("15/15/%0d/|=%b", iteration, a | b);
$display("15/15/%0d/^=%b", iteration, a ^ b);
$display("15/15/%0d/===%b", iteration, a == b);
$display("15/15/%0d/!==%b", iteration, a != b);
$display("15/15/%0d/====%b", iteration, a === b);
$display("15/15/%0d/!===%b", iteration, a !== b);
$display("15/15/%0d/<=%b", iteration, a < b);
$display("15/15/%0d/<==%b", iteration, a <= b);
$display("15/15/%0d/>=%b", iteration, a > b);
$display("15/15/%0d/>==%b", iteration, a >= b);
$display("15/15/%0d/&&=%b", iteration, a && b);
$display("15/15/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_16 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_16'(64'h8000000000000005); end
1: begin a=t_15'(32'h80000005); b=t_16'(64'h0); end
2: begin a=t_15'(32'h0); b=t_16'(64'hfffffffffffffffd); end
endcase
$display("15/16/%0d/+=%b", iteration, a + b);
$display("15/16/%0d/-=%b", iteration, a - b);
$display("15/16/%0d/*=%b", iteration, a * b);
$display("15/16/%0d//=%b", iteration, a / b);
$display("15/16/%0d/mod=%b", iteration, a % b);
$display("15/16/%0d/&=%b", iteration, a & b);
$display("15/16/%0d/|=%b", iteration, a | b);
$display("15/16/%0d/^=%b", iteration, a ^ b);
$display("15/16/%0d/===%b", iteration, a == b);
$display("15/16/%0d/!==%b", iteration, a != b);
$display("15/16/%0d/====%b", iteration, a === b);
$display("15/16/%0d/!===%b", iteration, a !== b);
$display("15/16/%0d/<=%b", iteration, a < b);
$display("15/16/%0d/<==%b", iteration, a <= b);
$display("15/16/%0d/>=%b", iteration, a > b);
$display("15/16/%0d/>==%b", iteration, a >= b);
$display("15/16/%0d/&&=%b", iteration, a && b);
$display("15/16/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_17 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_17'(64'h8000000000000005); end
1: begin a=t_15'(32'h80000005); b=t_17'(64'h0); end
2: begin a=t_15'(32'h0); b=t_17'(64'hfffffffffffffffd); end
endcase
$display("15/17/%0d/+=%b", iteration, a + b);
$display("15/17/%0d/-=%b", iteration, a - b);
$display("15/17/%0d/*=%b", iteration, a * b);
$display("15/17/%0d//=%b", iteration, a / b);
$display("15/17/%0d/mod=%b", iteration, a % b);
$display("15/17/%0d/&=%b", iteration, a & b);
$display("15/17/%0d/|=%b", iteration, a | b);
$display("15/17/%0d/^=%b", iteration, a ^ b);
$display("15/17/%0d/===%b", iteration, a == b);
$display("15/17/%0d/!==%b", iteration, a != b);
$display("15/17/%0d/====%b", iteration, a === b);
$display("15/17/%0d/!===%b", iteration, a !== b);
$display("15/17/%0d/<=%b", iteration, a < b);
$display("15/17/%0d/<==%b", iteration, a <= b);
$display("15/17/%0d/>=%b", iteration, a > b);
$display("15/17/%0d/>==%b", iteration, a >= b);
$display("15/17/%0d/&&=%b", iteration, a && b);
$display("15/17/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_18 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_18'(65'h10000000000000005); end
1: begin a=t_15'(32'h80000005); b=t_18'(65'h0); end
2: begin a=t_15'(32'h0); b=t_18'(65'h1fffffffffffffffd); end
endcase
$display("15/18/%0d/+=%b", iteration, a + b);
$display("15/18/%0d/-=%b", iteration, a - b);
$display("15/18/%0d/*=%b", iteration, a * b);
$display("15/18/%0d//=%b", iteration, a / b);
$display("15/18/%0d/mod=%b", iteration, a % b);
$display("15/18/%0d/&=%b", iteration, a & b);
$display("15/18/%0d/|=%b", iteration, a | b);
$display("15/18/%0d/^=%b", iteration, a ^ b);
$display("15/18/%0d/===%b", iteration, a == b);
$display("15/18/%0d/!==%b", iteration, a != b);
$display("15/18/%0d/====%b", iteration, a === b);
$display("15/18/%0d/!===%b", iteration, a !== b);
$display("15/18/%0d/<=%b", iteration, a < b);
$display("15/18/%0d/<==%b", iteration, a <= b);
$display("15/18/%0d/>=%b", iteration, a > b);
$display("15/18/%0d/>==%b", iteration, a >= b);
$display("15/18/%0d/&&=%b", iteration, a && b);
$display("15/18/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_19 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_19'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_19'(8'h0); end
2: begin a=t_15'(32'h0); b=t_19'(8'hfd); end
endcase
$display("15/19/%0d/+=%b", iteration, a + b);
$display("15/19/%0d/-=%b", iteration, a - b);
$display("15/19/%0d/*=%b", iteration, a * b);
$display("15/19/%0d//=%b", iteration, a / b);
$display("15/19/%0d/mod=%b", iteration, a % b);
$display("15/19/%0d/&=%b", iteration, a & b);
$display("15/19/%0d/|=%b", iteration, a | b);
$display("15/19/%0d/^=%b", iteration, a ^ b);
$display("15/19/%0d/===%b", iteration, a == b);
$display("15/19/%0d/!==%b", iteration, a != b);
$display("15/19/%0d/====%b", iteration, a === b);
$display("15/19/%0d/!===%b", iteration, a !== b);
$display("15/19/%0d/<=%b", iteration, a < b);
$display("15/19/%0d/<==%b", iteration, a <= b);
$display("15/19/%0d/>=%b", iteration, a > b);
$display("15/19/%0d/>==%b", iteration, a >= b);
$display("15/19/%0d/&&=%b", iteration, a && b);
$display("15/19/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_20 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_20'(16'h8005); end
1: begin a=t_15'(32'h80000005); b=t_20'(16'h0); end
2: begin a=t_15'(32'h0); b=t_20'(16'hfffd); end
endcase
$display("15/20/%0d/+=%b", iteration, a + b);
$display("15/20/%0d/-=%b", iteration, a - b);
$display("15/20/%0d/*=%b", iteration, a * b);
$display("15/20/%0d//=%b", iteration, a / b);
$display("15/20/%0d/mod=%b", iteration, a % b);
$display("15/20/%0d/&=%b", iteration, a & b);
$display("15/20/%0d/|=%b", iteration, a | b);
$display("15/20/%0d/^=%b", iteration, a ^ b);
$display("15/20/%0d/===%b", iteration, a == b);
$display("15/20/%0d/!==%b", iteration, a != b);
$display("15/20/%0d/====%b", iteration, a === b);
$display("15/20/%0d/!===%b", iteration, a !== b);
$display("15/20/%0d/<=%b", iteration, a < b);
$display("15/20/%0d/<==%b", iteration, a <= b);
$display("15/20/%0d/>=%b", iteration, a > b);
$display("15/20/%0d/>==%b", iteration, a >= b);
$display("15/20/%0d/&&=%b", iteration, a && b);
$display("15/20/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_21 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_21'(8'h85); end
1: begin a=t_15'(32'h80000005); b=t_21'(8'h0); end
2: begin a=t_15'(32'h0); b=t_21'(8'hfd); end
endcase
$display("15/21/%0d/+=%b", iteration, a + b);
$display("15/21/%0d/-=%b", iteration, a - b);
$display("15/21/%0d/*=%b", iteration, a * b);
$display("15/21/%0d//=%b", iteration, a / b);
$display("15/21/%0d/mod=%b", iteration, a % b);
$display("15/21/%0d/&=%b", iteration, a & b);
$display("15/21/%0d/|=%b", iteration, a | b);
$display("15/21/%0d/^=%b", iteration, a ^ b);
$display("15/21/%0d/===%b", iteration, a == b);
$display("15/21/%0d/!==%b", iteration, a != b);
$display("15/21/%0d/====%b", iteration, a === b);
$display("15/21/%0d/!===%b", iteration, a !== b);
$display("15/21/%0d/<=%b", iteration, a < b);
$display("15/21/%0d/<==%b", iteration, a <= b);
$display("15/21/%0d/>=%b", iteration, a > b);
$display("15/21/%0d/>==%b", iteration, a >= b);
$display("15/21/%0d/&&=%b", iteration, a && b);
$display("15/21/%0d/||=%b", iteration, a || b);
end end
begin t_15 a; t_22 b;
for (int iteration=0; iteration<3; iteration++) begin case (iteration)
0: begin a=t_15'(32'hfffffffd); b=t_22'(16'h8005); end
1: begin a=t_15'(32'h80000005); b=t_22'(16'h0); end
2: begin a=t_15'(32'h0); b=t_22'(16'hfffd); end
endcase
$display("15/22/%0d/+=%b", iteration, a + b);
$display("15/22/%0d/-=%b", iteration, a - b);
$display("15/22/%0d/*=%b", iteration, a * b);
$display("15/22/%0d//=%b", iteration, a / b);
$display("15/22/%0d/mod=%b", iteration, a % b);
$display("15/22/%0d/&=%b", iteration, a & b);
$display("15/22/%0d/|=%b", iteration, a | b);
$display("15/22/%0d/^=%b", iteration, a ^ b);
$display("15/22/%0d/===%b", iteration, a == b);
$display("15/22/%0d/!==%b", iteration, a != b);
$display("15/22/%0d/====%b", iteration, a === b);
$display("15/22/%0d/!===%b", iteration, a !== b);
$display("15/22/%0d/<=%b", iteration, a < b);
$display("15/22/%0d/<==%b", iteration, a <= b);
$display("15/22/%0d/>=%b", iteration, a > b);
$display("15/22/%0d/>==%b", iteration, a >= b);
$display("15/22/%0d/&&=%b", iteration, a && b);
$display("15/22/%0d/||=%b", iteration, a || b);
end end
$finish(0); end endmodule
