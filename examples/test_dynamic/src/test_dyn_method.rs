#![allow(unused)]
use alanthinker_dynamic_get_field_macro::*;
use alanthinker_dynamic_get_field_trait::*;
use anyhow::*;
use std::{
    any::{self, Any},
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::Rc,
};

#[derive(Debug)]
struct Object1 {
    value: i32,
}

#[derive(Debug)]
struct Calculator {
    value: i32,
}

// 不希望被宏影响的方法放在没有被 #[dynamic_methods] 标记的 impl 块中
impl Calculator {
    pub fn some_fn(&self) {
        //
    }
}

// 被 #[dynamic_methods] 标记的 impl 快中, 每个方法都可以被动态调用.
// 目前对方法有2个限制,
// 1. 参数如果没实现 Copy, 那么就只能用引用传递. 如果确实想传递变量, 可以考虑用 &Rc<RefCell<T>> 或者 &Arc<Mutex<T>>
// 2. 除了 &mut self 外, 其他所有参数不能为 mut. 如果确实有这个需求, 用 &Rc<RefCell<T>> 或者 &Arc<Mutex<T>
#[dynamic_methods]
impl Calculator {
    pub fn get_value(&self) -> i32 {
        println!("Calculator get_value called");
        self.value
    }

    pub fn set_value(&mut self, value: i32) {
        println!("Calculator set_value called");
        self.value = value;
    }

    pub fn add(&mut self, x: i32) -> i32 {
        println!("Calculator add called");
        self.value += x;
        self.value
    }

    // //注意, 参数如果没实现 Copy, 那么就只能用引用传递. 如果确实想传递变量, 可以考虑用 &Rc<RefCell<T>> 或者 &Arc<Mutex<T>>
    // pub fn operation_abc(&self, ob: Object1, x: i32) -> i32 {
    //     println!("Calculator add called");
    //     //self.value += x;
    //     ob.value
    // }

    pub fn operation_ref(&self, ob: &Object1, x: i32) -> i32 {
        println!("Calculator add called");
        //self.value += x;
        ob.value
    }

    //  宏 dynamic_methods, 除了 &mut self 外, 支持 mut 参数的难度极高, 也没必要. 如果确实有这个需求, 用 &Rc<RefCell<T>> 或者 &Arc<Mutex<T>>
    pub fn operation_change_arg_value(&self, ob: &Rc<RefCell<Object1>>, x: i32) -> i32 {
        println!("Calculator add called");
        let mut ob = ob.borrow_mut();
        ob.value += self.value;
        ob.value += x;
        ob.value
    }

    pub fn get_static(c: &Object1, x: i32) -> i32 {
        println!("Calculator get_static called");
        c.value
    }

    //  宏 dynamic_methods, 除了  &mut self 外, 支持 mut 参数的难度极高, 也没必要. 如果确实有这个需求, 用 Rc<RefCell<T>> 或者 Arc<Mutex<T>>
    pub fn static_change_arg_value(c: &Rc<RefCell<Object1>>, x: i32) -> i32 {
        println!("Calculator get_static called");
        let mut c = c.borrow_mut();
        c.value += x;
        c.value
    }
}

#[test]
fn test_call_method1() -> Result<()> {
    let calc = Calculator { value: 21 };

    // 测试不可变调用
    let result = call::call_and_downcast::<Calculator, i32>("get_value", &calc, &[])?;
    assert_eq!(result, 21);

    // 测试可变调用
    let mut calc = Calculator { value: 21 };
    call::call_mut("set_value", &mut calc, &[&100])?;
    assert_eq!(calc.value, 100);

    // 测试带参数调用
    let result = call::call_mut_and_downcast::<Calculator, i32>("add", &mut calc, &[&50])?;
    assert_eq!(result, 150);
    assert_eq!(calc.value, 150);

    // 测试错误：用不可变引用调用可变方法
    let calc_ref = &calc;
    let result = call::try_call("set_value", calc_ref, &[&200]);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("requires mutable reference"));

    let ob1 = Object1 { value: 53 };

    // let result = call::call_and_downcast::<Calculator, i32>("operation", &calc, &[&ob1, &26])?;
    // assert_eq!(result, 53);

    let result = call::call_static_and_downcast::<Calculator, i32>("get_static", &[&ob1, &53])?;
    assert_eq!(result, 53);

    let ob1 = Rc::new(RefCell::new(Object1 { value: 53 }));
    let result = call::call_and_downcast::<Calculator, i32>(
        "operation_change_arg_value",
        &calc,
        &[&ob1, &26],
    )?;
    assert_eq!(result, 229);

    let result =
        call::call_static_and_downcast::<Calculator, i32>("static_change_arg_value", &[&ob1, &26])?;
    assert_eq!(result, 255);

    Ok(())
}

#[test]
fn test_find_method() -> Result<()> {
    // 测试查找方法
    let method = find::find_method::<Calculator>("get_value")?;
    assert_eq!(method.name(), "get_value");
    assert!(method.is_immutable());

    let method = find::find_mutable_method::<Calculator>("set_value")?;
    assert_eq!(method.name(), "set_value");
    assert!(method.is_mutable());

    // 测试方法不存在的情况
    let result = find::find_method::<Calculator>("non_existent");
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("not found"));

    Ok(())
}

#[derive(Debug)]
struct Calculator2 {
    value: i32,
}

#[dynamic_methods]
impl Calculator2 {
    pub fn get_value(&self) -> i32 {
        println!("Calculator2 called");
        self.value
    }

    pub fn set_value(&mut self, value: i32) {
        println!("Calculator2 set_value called");
        self.value = value;
    }

    pub fn add(&mut self, x: i32) -> i32 {
        println!("Calculator2 add called");
        self.value += x;
        self.value
    }
}

#[test]
fn test_call_method2() -> Result<()> {
    let calc = Calculator2 { value: 21 };

    // 测试不可变调用
    let result = call::call_and_downcast::<Calculator2, i32>("get_value", &calc, &[])?;
    assert_eq!(result, 21);

    // 测试可变调用
    let mut calc = Calculator2 { value: 21 };
    call::call_mut("set_value", &mut calc, &[&100])?;
    assert_eq!(calc.value, 100);

    // 测试带参数调用
    let result = call::call_mut_and_downcast::<Calculator2, i32>("add", &mut calc, &[&50])?;
    assert_eq!(result, 150);
    assert_eq!(calc.value, 150);

    // 测试错误：用不可变引用调用可变方法
    let calc_ref = &calc;
    let result = call::try_call("set_value", calc_ref, &[&200]);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("requires mutable reference"));

    Ok(())
}

#[test]
fn test_util() -> Result<()> {
    let calc = Calculator { value: 10 };

    // 测试链式调用
    let chain = util::MethodChain::new(&calc)
        .call("get_value", vec![])
        .call("get_value", vec![]);

    let results = chain.execute()?;
    assert_eq!(results.len(), 2);

    // 测试不可变动态调用器
    let invoker = util::DynamicInvoker::new(&calc);
    let value: i32 = invoker.invoke_as("get_value", &[])?;
    assert_eq!(value, 10);

    // 检查方法可调用性
    assert!(invoker.can_invoke("get_value"));
    assert!(!invoker.can_invoke("set_value")); // 需要可变引用

    // 测试可变动态调用器
    let mut calc = Calculator { value: 10 };
    let mut invoker_mut = util::DynamicInvokerMut::new(&mut calc);
    assert!(invoker_mut.can_invoke("set_value")); // 现在可以调用可变方法

    // 调用可变方法
    invoker_mut.invoke("set_value", &[&20])?;
    assert_eq!(calc.value, 20);

    // 测试通用动态调用器
    let calc_ref = &calc;
    let mut caller = util::DynamicCaller::new(calc_ref);
    let value: i32 = caller.invoke_as("get_value", &[])?;
    assert_eq!(value, 20);

    let mut calc_mut = Calculator { value: 30 };

    let mut caller_mut = util::DynamicCaller::new_mut(&mut calc_mut);
    caller_mut.invoke("set_value", &[&40])?;
    let value: i32 = caller_mut.invoke_as("get_value", &[])?;
    assert_eq!(value, 40);

    // 测试转换为不可变调用器
    let immutable_invoker = caller_mut.as_immutable();
    let value: i32 = immutable_invoker.invoke_as("get_value", &[])?;
    assert_eq!(value, 40);

    Ok(())
}

#[test]
fn test_error_handling() -> Result<()> {
    let calc = Calculator { value: 21 };

    // 测试类型转换错误
    let result = call::call_and_downcast::<Calculator, String>("get_value", &calc, &[]);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    //println!("error_msg={}", error_msg);
    assert!(error_msg.contains("Failed to downcast"));
    assert!(error_msg.contains("alloc::string::String"));

    // 测试参数错误
    let mut calc = Calculator { value: 21 };
    let result = call::call_mut("add", &mut calc, &[&"not_a_number"]);
    assert!(result.is_err());

    // 测试批量调用中的错误传播
    let calls = vec![
        ("get_value", vec![] as Vec<&dyn Any>),
        ("non_existent", vec![]),
        ("get_value", vec![]),
    ];

    let result = util::batch_call(&calc, &calls);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    //println!("error_msg={}", error_msg);
    assert!(error_msg.contains("Failed to call method"));
    assert!(error_msg.contains("non_existent"));

    Ok(())
}
