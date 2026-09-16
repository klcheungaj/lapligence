import argparse
import ctypes as C
from pathlib import Path
import random

parser = argparse.ArgumentParser(description="Independent integer checks and optional stage-01 C differential")
parser.add_argument("--dynamic", required=True, type=Path)
parser.add_argument("--legacy", type=Path, help="Stage-01 value library built with LLG_MODEL_MAX_WIDTH=1024")
options = parser.parse_args()
for path in (options.dynamic, options.legacy):
    if path is not None and not path.is_file():
        parser.error(f"Library does not exist: {path}")

rng=random.Random(27427)
class D(C.Structure):
 _fields_=[('bits',C.POINTER(C.c_uint64)),('x',C.POINTER(C.c_uint64)),('z',C.POINTER(C.c_uint64)),('width',C.c_uint32),('is_signed',C.c_int8)]
class L(C.Structure):
 _fields_=[('bits',C.c_uint64*16),('x',C.c_uint64*16),('z',C.c_uint64*16),('width',C.c_uint32),('is_signed',C.c_int8)]
d = C.CDLL(str(options.dynamic.resolve()))
l = C.CDLL(str(options.legacy.resolve())) if options.legacy else None
for lib,T in [(d,D)] + ([(l,L)] if l is not None else []):
 lib.sv4_from_limbs.argtypes=[C.POINTER(C.c_uint64)]*3+[C.c_uint32,C.c_int8];lib.sv4_from_limbs.restype=T
 for name in ['add','sub','mul','div','mod','pow','and','or','xor','xnor','eq','neq','case_eq','case_neq','wild_eq','wild_neq','casez_eq','casex_eq','lt','le','gt','ge','shl','shr','ashl','ashr','logand','logor','logimpl','logequiv']:
  f=getattr(lib,'sv4_'+name);f.argtypes=[T,T];f.restype=T
 for name in ['neg','bitneg','lognot','reduce_and','reduce_or','reduce_xor','reduce_nand','reduce_nor','reduce_xnor','clog2','countones','to_two_state','repeat_count']:
  f=getattr(lib,'sv4_'+name);f.argtypes=[T];f.restype=T
 lib.sv4_to_real.argtypes=[T];lib.sv4_to_real.restype=C.c_double
 lib.sv4_to_dec_string.argtypes=[T,C.c_char_p,C.c_size_t]
 lib.sv4_resize.argtypes=[T,C.c_uint32,C.c_int8];lib.sv4_resize.restype=T
 lib.sv4_cast.argtypes=[T,C.c_uint32,C.c_int8];lib.sv4_cast.restype=T
 lib.sv4_mux.argtypes=[T,T,T];lib.sv4_mux.restype=T
 lib.sv4_stream.argtypes=[T,C.c_uint32,C.c_int];lib.sv4_stream.restype=T
 lib.sv4_unstream.argtypes=[T,C.c_uint32,C.c_int];lib.sv4_unstream.restype=T
 lib.sv4_part_select.argtypes=[T,C.c_int64,C.c_int64];lib.sv4_part_select.restype=T
 lib.sv4_part_select_set.argtypes=[C.POINTER(T),C.c_int64,C.c_int64,T]
d.sv4_destroy.argtypes=[C.POINTER(D)]
def mk(lib,parts,w,s):
 arrays=[(C.c_uint64*((w+63)//64))(*[(p>>(64*i))&((1<<64)-1) for i in range((w+63)//64)]) for p in parts]
 return lib.sv4_from_limbs(*arrays,w,s)
def vals(v):
 return (v.width,v.is_signed,*[sum(int(getattr(v,k)[i])<<(64*i) for i in range((v.width+63)//64)) for k in ['bits','x','z']])
count=0
for rep in range(1800 if l is not None else 0):
 wa=rng.choice([0,1,2,8,31,32,33,63,64,65,127,128,129,256,257,512])
 wb=rng.choice([0,1,2,8,31,32,33,63,64,65,127,128,129,256,257,512])
 sa=rng.randrange(2);sb=rng.randrange(2)
 pa=[rng.getrandbits(wa),0,0];pb=[rng.getrandbits(wb),0,0]
 if rep%3==0:
  pa=[rng.getrandbits(wa) for _ in range(3)];pb=[rng.getrandbits(wb) for _ in range(3)]
  pa[2]&=~pa[1];pb[2]&=~pb[1]
 if rep%7==0:pb=[rng.randrange(100),0,0]
 a=mk(d,pa,wa,sa);b=mk(d,pb,wb,sb);aa=mk(l,pa,wa,sa);bb=mk(l,pb,wb,sb)
 initiala=vals(a);initialb=vals(b)
 for name in ['add','sub','mul','div','mod','pow','and','or','xor','xnor','eq','neq','case_eq','case_neq','wild_eq','wild_neq','casez_eq','casex_eq','lt','le','gt','ge','shl','shr','ashl','ashr','logand','logor','logimpl','logequiv']:
  r=getattr(d,'sv4_'+name)(a,b);rr=getattr(l,'sv4_'+name)(aa,bb)
  assert vals(r)==vals(rr),(rep,name,initiala,initialb,vals(r),vals(rr))
  assert vals(a)==initiala and vals(b)==initialb,(name,'mutation')
  d.sv4_destroy(C.byref(r));count+=1
 for name in ['neg','bitneg','lognot','reduce_and','reduce_or','reduce_xor','reduce_nand','reduce_nor','reduce_xnor','clog2','countones','to_two_state','repeat_count']:
  r=getattr(d,'sv4_'+name)(a);rr=getattr(l,'sv4_'+name)(aa)
  assert vals(r)==vals(rr),(rep,name,initiala,vals(r),vals(rr))
  assert vals(a)==initiala,(name,'mutation')
  d.sv4_destroy(C.byref(r));count+=1
 for name in ['resize','cast']:
  r=getattr(d,'sv4_'+name)(a,wb,sb);rr=getattr(l,'sv4_'+name)(aa,wb,sb)
  assert vals(r)==vals(rr),(rep,name,vals(r),vals(rr))
  d.sv4_destroy(C.byref(r));count+=1
 for name in ['stream','unstream']:
  sl=rng.randrange(1,1000);dr=rng.randrange(2)
  r=getattr(d,'sv4_'+name)(a,sl,dr);rr=getattr(l,'sv4_'+name)(aa,sl,dr)
  assert vals(r)==vals(rr),(rep,name)
  d.sv4_destroy(C.byref(r));count+=1
 buf=C.create_string_buffer(1024);buf2=C.create_string_buffer(1024)
 d.sv4_to_dec_string(a,buf,1024);l.sv4_to_dec_string(aa,buf2,1024)
 assert buf.value==buf2.value,('decimal',initiala,buf.value,buf2.value)
 out=d.sv4_to_real(a);old=l.sv4_to_real(aa)
 assert out==old,('real',initiala,out,old)
 assert vals(a)==initiala,('conversion','mutation')
 d.sv4_destroy(C.byref(a));d.sv4_destroy(C.byref(b))
if l is not None:
    print('Legacy semantic differential:',count,'value results matched; operands unchanged')
for i in range(7000):
 w=rng.choice([1,31,32,33,63,64,65,127,128,129,257,511,512,513,1023])
 mask=(1<<w)-1;na=rng.getrandbits(w);nb=rng.getrandbits(rng.randrange(1,w+1)) or 1
 a=mk(d,[na,0,0],w,0);b=mk(d,[nb,0,0],w,0)
 for op,expected in [('add',(na+nb)&mask),('sub',(na-nb)&mask),('mul',(na*nb)&mask),('div',na//nb),('mod',na%nb)]:
  result=getattr(d,'sv4_'+op)(a,b)
  assert vals(result)==(w,0,expected,0,0),(op,w,na,nb,vals(result),expected)
  d.sv4_destroy(C.byref(result))
 d.sv4_destroy(C.byref(a));d.sv4_destroy(C.byref(b))
print('Independent integer oracle:',7000*5,'checks passed')
